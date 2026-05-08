use rusqlite::Connection;
use std::collections::HashMap;

pub struct SessionImport {
    pub id: String,
    pub source_name: String,
}

pub struct SessionReader<'a> {
    conn: &'a Connection,
}

impl<'a> SessionReader<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn list_imports(&self) -> Result<Vec<SessionImport>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, source_name FROM session_imports ORDER BY imported_at ASC")
            .map_err(|e| e.to_string())?;

        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|(id, source_name)| SessionImport { id, source_name })
            .collect())
    }

    pub fn get_rows(&self, import_id: &str) -> Result<Vec<HashMap<String, String>>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT data_json FROM session_rows \
                 WHERE import_id = ?1 ORDER BY row_index ASC",
            )
            .map_err(|e| e.to_string())?;

        let jsons: Vec<String> = stmt
            .query_map(rusqlite::params![import_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        jsons
            .into_iter()
            .map(|json| {
                serde_json::from_str::<HashMap<String, String>>(&json)
                    .map_err(|e| format!("Corrupt session row: {e}"))
            })
            .collect()
    }
}
