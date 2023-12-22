use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{crate_name, crate_version, Parser};
use tokio::fs;
use tracing::level_filters::LevelFilter;
use tracing::{info, instrument};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use data::v6::GrowattV6EnergyFragment;

use crate::config::Config;
use crate::misc::run_decrypt;
use crate::server::run_server;

#[macro_use]
mod utils;
mod config;
mod data;
mod data_message;
mod misc;
mod server;

const BUF_SIZE: usize = 65535;

#[tokio::main]
#[instrument]
async fn main() -> Result<()> {
    // First thing: load the arguments and configuration file.
    let cli = Cli::parse();

    println!(
        "{} starting up, looking for configuration file",
        crate_name!()
    );
    let config_path: &str = cli.config_path.as_str();

    let config = log_error!(config::load_from_yaml(config_path).await.context(format!(
        "Failed to load the configuration file from {}",
        config_path
    )))?;

    // Set up logging
    let _logger_guard = init_logging(&cli, &config)?;

    // Finally starting!
    info!(
        "

    {} version {} started!
    Copyright (C) 2023-2024 - Stefano Vittorio Porta

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
    ",
        crate_name!(),
        crate_version!()
    );

    // Inverter specifications loading
    if config.inverters_dir.is_none() {
        info!("No inverters path specified. Using default");
    }
    let inverters_dir = config
        .inverters_dir
        .clone()
        .unwrap_or("./inverters/Growatt v6.yaml".to_string());

    #[derive(serde::Deserialize)]
    struct MappingFile {
        mappings: Vec<GrowattV6EnergyFragment>,
    }

    let mappings_string =
        log_error!(fs::read_to_string(&inverters_dir)
            .await
            .with_context(|| format!(
                "Could not load inverters definitions from {}",
                &inverters_dir
            )))?;
    let mapping_file: MappingFile = log_error!(serde_yaml::from_str(&mappings_string))?;
    let inverter: Arc<Vec<GrowattV6EnergyFragment>> = Arc::new(mapping_file.mappings);

    let res = match cli.subcommand {
        Commands::Start => run_server(config, inverter).await,
        Commands::Decrypt { file_path } => run_decrypt(file_path, config, inverter).await,
    };

    log_error!(res)
}

fn init_logging(cli: &Cli, config: &Arc<Config>) -> Result<WorkerGuard> {
    let logging_options = config.logging.as_ref();

    let base_logging = if let Some(level) = &cli.logging_level {
        // If the CLI overrides the logging level, use the specified level
        level.clone().into()
    } else if let Some(level) = logging_options.as_ref().and_then(|l| l.level.as_ref()) {
        // If a logging level is specified in the config file, use that
        let file_level = LevelFilter::from_str(level);
        if let Ok(file_level) = file_level {
            file_level
        } else {
            // If the level is invalid, use INFO as the default level
            eprintln!(
                "WARNING: Invalid logging level specified in the config file: {}. Using INFO instead",
                level
            );
            LevelFilter::INFO
        }
    } else {
        // Otherwise use INFO as the default level
        LevelFilter::INFO
    };

    let console_logging_filter = EnvFilter::builder()
        .with_default_directive(base_logging.into())
        .with_env_var("LOG_LEVEL")
        .from_env_lossy();

    // this can fail, todo
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::HOURLY)
        .filename_prefix("growatt_server")
        .filename_suffix("log")
        .build(
            logging_options
                .and_then(|l| l.directory.clone())
                .unwrap_or("./logs".to_string()),
        )
        .context("Initializing rolling file appender failed")?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(console_logging_filter)
        .with(fmt::layer().with_writer(non_blocking))
        .init();

    // Hook to log also panics with tracing
    std::panic::set_hook(Box::new(panic_hook));

    Ok(guard)
}

#[derive(clap::Parser)]
#[clap(
    name = "growatt_server",
    version,
    author,
    about,
    subcommand_required = true
)]
struct Cli {
    /// Path to the config file to use to run the server
    #[clap(short, long, default_value = "./config.yaml")]
    config_path: String,

    /// Override the logging level.
    #[clap(short, long, value_enum)]
    logging_level: Option<LogLevel>,

    #[clap(subcommand)]
    subcommand: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Starts the server
    Start,
    /// Decrypt one or more messages. Won't start the server
    Decrypt {
        /// Path to the file containing the messages to decrypt
        #[clap(short, long, default_value = "./messages.json")]
        file_path: String,
    },
}

#[derive(clap::ValueEnum, Clone)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}
