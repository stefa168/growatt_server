use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub inverters_dir: Option<String>,
    pub db: DbConfig,
    pub listen_port: Option<u16>,
    pub remote_port: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DbConfig {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database: String,
}

pub async fn load_from_yaml(path: &str) -> Result<Config, String> {
    let yaml = fs::read_to_string(path).await.map_err(|e| e.to_string())?;
    let config: Config = serde_yaml::from_str(&yaml).map_err(|e| e.to_string())?;
    Ok(config)
}
