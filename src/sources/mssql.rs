use crate::schema::{Column, DataType, Table};
use crate::sources::DataSource;
use anyhow::{Context, Result};
use tiberius::{Client, Config, Query};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub struct MssqlSource {
    connection_string: String,
    client: Option<Client<Compat<TcpStream>>>,
}

impl MssqlSource {
    pub fn new(connection_string: String) -> Self {
        Self {
            connection_string,
            client: None,
        }
    }

    async fn connect(&mut self) -> Result<()> {
        let config = Config::from_ado_string(&self.connection_string)
            .context("Failed to parse connection string")?;

        let tcp = TcpStream::connect(config.get_addr())
            .await
            .context("Failed to connect to SQL Server")?;
        tcp.set_nodelay(true)?;

        let client = Client::connect(config, tcp.compat_write())
            .await
            .context("Failed to authenticate with SQL Server")?;

        self.client = Some(client);
        Ok(())
    }

    async fn get_tables(&mut self) -> Result<Vec<(String, String)>> {
        let client = self.client.as_mut().context("Not connected")?;

        let query = r#"
            SELECT
                TABLE_SCHEMA,
                TABLE_NAME
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_TYPE = 'BASE TABLE'
            ORDER BY TABLE_SCHEMA, TABLE_NAME
        "#;

        let stream = Query::new(query).query(client).await?;
        let rows = stream.into_first_result().await?;

        let mut tables = Vec::new();
        for row in rows {
            let schema: &str = row.get(0).context("Missing TABLE_SCHEMA")?;
            let name: &str = row.get(1).context("Missing TABLE_NAME")?;
            tables.push((schema.to_string(), name.to_string()));
        }

        Ok(tables)
    }

    async fn get_columns(&mut self, schema: &str, table: &str) -> Result<Vec<Column>> {
        let client = self.client.as_mut().context("Not connected")?;

        let query = format!(
            r#"
            SELECT
                c.COLUMN_NAME,
                c.DATA_TYPE,
                c.IS_NULLABLE,
                c.CHARACTER_MAXIMUM_LENGTH,
                c.NUMERIC_PRECISION,
                c.NUMERIC_SCALE,
                c.COLUMN_DEFAULT,
                CASE WHEN pk.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS IS_PRIMARY_KEY,
                CASE WHEN fk.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS IS_FOREIGN_KEY
            FROM INFORMATION_SCHEMA.COLUMNS c
            LEFT JOIN (
                SELECT ku.TABLE_SCHEMA, ku.TABLE_NAME, ku.COLUMN_NAME
                FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc
                JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE ku
                    ON tc.CONSTRAINT_NAME = ku.CONSTRAINT_NAME
                WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY'
            ) pk ON c.TABLE_SCHEMA = pk.TABLE_SCHEMA
                AND c.TABLE_NAME = pk.TABLE_NAME
                AND c.COLUMN_NAME = pk.COLUMN_NAME
            LEFT JOIN (
                SELECT ku.TABLE_SCHEMA, ku.TABLE_NAME, ku.COLUMN_NAME
                FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc
                JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE ku
                    ON tc.CONSTRAINT_NAME = ku.CONSTRAINT_NAME
                WHERE tc.CONSTRAINT_TYPE = 'FOREIGN KEY'
            ) fk ON c.TABLE_SCHEMA = fk.TABLE_SCHEMA
                AND c.TABLE_NAME = fk.TABLE_NAME
                AND c.COLUMN_NAME = fk.COLUMN_NAME
            WHERE c.TABLE_SCHEMA = '{}' AND c.TABLE_NAME = '{}'
            ORDER BY c.ORDINAL_POSITION
            "#,
            schema, table
        );

        let stream = Query::new(query).query(client).await?;
        let rows = stream.into_first_result().await?;

        let mut columns = Vec::new();
        for row in rows {
            let name: &str = row.get(0).context("Missing COLUMN_NAME")?;
            let data_type_str: &str = row.get(1).context("Missing DATA_TYPE")?;
            let is_nullable: &str = row.get(2).context("Missing IS_NULLABLE")?;
            let max_length: Option<i32> = row.get(3);
            let precision: Option<u8> = row.get(4);
            let scale: Option<u8> = row.get(5);
            let default_value: Option<&str> = row.get(6);
            let is_pk: i32 = row.get(7).unwrap_or(0);
            let is_fk: i32 = row.get(8).unwrap_or(0);

            let data_type = Self::parse_data_type(data_type_str, max_length, precision, scale);

            let mut column = Column::new(name.to_string(), data_type)
                .with_nullable(is_nullable == "YES")
                .with_primary_key(is_pk == 1)
                .with_foreign_key(is_fk == 1);

            column.default_value = default_value.map(|s| s.to_string());
            column.max_length = max_length.map(|l| l as usize);
            column.precision = precision;
            column.scale = scale;

            columns.push(column);
        }

        Ok(columns)
    }

    fn parse_data_type(
        type_name: &str,
        max_length: Option<i32>,
        precision: Option<u8>,
        scale: Option<u8>,
    ) -> DataType {
        match type_name.to_lowercase().as_str() {
            "int" => DataType::Integer,
            "bigint" => DataType::BigInt,
            "smallint" => DataType::SmallInt,
            "tinyint" => DataType::TinyInt,
            "decimal" | "numeric" => DataType::Decimal {
                precision: precision.unwrap_or(18),
                scale: scale.unwrap_or(0),
            },
            "float" => DataType::Float,
            "real" => DataType::Real,
            "char" => DataType::Char {
                length: max_length.unwrap_or(1) as usize,
            },
            "varchar" => DataType::VarChar {
                max_length: if let Some(l) = max_length {
                    if l == -1 {
                        None
                    } else {
                        Some(l as usize)
                    }
                } else {
                    None
                },
            },
            "nvarchar" => DataType::VarChar {
                max_length: if let Some(l) = max_length {
                    if l == -1 {
                        None
                    } else {
                        Some((l / 2) as usize)
                    }
                } else {
                    None
                },
            },
            "text" | "ntext" => DataType::Text,
            "date" => DataType::Date,
            "datetime" => DataType::DateTime,
            "datetime2" => DataType::DateTime2,
            "time" => DataType::Time,
            "timestamp" | "rowversion" => DataType::Timestamp,
            "binary" => DataType::Binary {
                length: max_length.unwrap_or(1) as usize,
            },
            "varbinary" => DataType::VarBinary {
                max_length: if let Some(l) = max_length {
                    if l == -1 {
                        None
                    } else {
                        Some(l as usize)
                    }
                } else {
                    None
                },
            },
            "bit" => DataType::Boolean,
            "uniqueidentifier" => DataType::Uuid,
            "xml" => DataType::Xml,
            _ => DataType::Unknown(type_name.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl DataSource for MssqlSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>> {
        self.connect().await?;

        let tables_list = self.get_tables().await?;
        let mut tables = Vec::new();

        for (schema, table_name) in tables_list {
            let columns = self.get_columns(&schema, &table_name).await?;

            let mut table = Table::new(
                table_name.clone(),
                "mssql".to_string(),
                self.connection_string.clone(),
            )
            .with_schema(schema.clone());

            for column in columns {
                table.add_column(column);
            }

            tables.push(table);
        }

        Ok(tables)
    }

    fn source_type(&self) -> &str {
        "mssql"
    }
}
