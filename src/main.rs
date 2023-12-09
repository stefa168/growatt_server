use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{arg, crate_authors, crate_description, crate_name, crate_version, Command};
use data::v6::GrowattV6EnergyFragment;
use tokio::fs;
use tracing::level_filters::LevelFilter;
use tracing::{info, instrument};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

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
    let args = get_cli_conf().get_matches();

    println!(
        "{} starting up, looking for configuration file",
        crate_name!()
    );
    let config_path: &String = args.get_one("config_path").unwrap();

    let config = log_error!(config::load_from_yaml(config_path).await.context(format!(
        "Failed to load the configuration file from {}",
        config_path
    )))?;

    // Set up logging
    let _logger_guard = init_logging(&config)?;

    // Finally starting!
    info!("{} version {} started.", crate_name!(), crate_version!());

    // Inverter specifications loading
    if config.inverters_dir.is_none() {
        info!("No inverters path specified. Using default");
    }
    let inverters_dir = config
        .inverters_dir
        .clone()
        .unwrap_or("./inverters/Growatt v6.json".to_string());

    let json = log_error!(fs::read_to_string(&inverters_dir)
        .await
        .with_context(|| format!(
            "Could not load inverters definitions from {}",
            &inverters_dir
        )))?;

    let inverter: Arc<Vec<GrowattV6EnergyFragment>> =
        Arc::new(log_error!(serde_json::from_str(&json))?);

    let (subcommand_name, more_args) = args.subcommand().unwrap();

    log_error!(match subcommand_name {
        "start" => run_server(config, inverter).await,
        "decrypt" => run_decrypt(more_args, config, inverter).await,
        _ => bail!("Unknown subcommand. (How did we arrive here?)"),
    })
}

fn init_logging(config: &Config) -> Result<WorkerGuard> {
    let options = config.logging.as_ref();

    let base_logging = LevelFilter::from_str(
        &options
            .and_then(|logging| logging.level.clone())
            .unwrap_or("info".to_string()),
    )
    .unwrap();

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
            options
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

fn get_cli_conf() -> Command {
    Command::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .subcommand_required(true)
        .arg(
            arg!(config_path: -c --config_path <PATH> "Path to configuration file")
                .help("Path to the config file to use to run the server")
                .default_value("./config.yaml"),
        )
        .subcommand(Command::new("start").about("Starts the server"))
        .subcommand(
            Command::new("decrypt")
                .about("Decrypt one or more messages. Won't start the server")
                .arg(
                    arg!(file: -f --file <PATH> "Path to file with the messages to decrypt")
                        .required(true)
                        .default_value("./messages.json"),
                ),
        )
}
