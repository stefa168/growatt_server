use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub inverters_dir: Option<String>,
    #[serde(alias = "db")]
    pub database: DbConfig,
    pub listen_port: Option<u16>,
    pub remote_address: Option<String>,
    pub logging: Option<Logging>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Logging {
    #[serde(alias = "log_level")]
    pub level: Option<String>,
    #[serde(alias = "log_directory")]
    pub directory: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DbConfig {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database: String,
}

pub async fn load_from_yaml(path: &str) -> Result<Arc<Config>> {
    let yaml = fs::read_to_string(path).await?;
    let config = serde_yaml::from_str(&yaml)?;
    Ok(Arc::new(config))
}
