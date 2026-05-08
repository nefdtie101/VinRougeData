use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WorkerConfig {
    pub port: u16,
    pub data_dir: String,
}

impl WorkerConfig {
    pub fn load() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("VINROUGE"))
            .set_default("port", 3000)?
            .set_default("data_dir", "./projects")?
            .build()?;
        Ok(cfg.try_deserialize()?)
    }
}
