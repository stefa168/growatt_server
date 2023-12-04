use std::io;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{arg, crate_authors, crate_description, crate_name, crate_version, ArgMatches, Command};
use data::GrowattV6EnergyFragment;
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::signal::unix::SignalKind;
use tokio::task::JoinHandle;
use tokio::{fs, signal};
use tracing::level_filters::LevelFilter;
use tracing::{error, info, instrument, span, Level};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use crate::config::Config;
use crate::utils::LogError;

mod config;
mod data;
mod data_message;
mod server;
mod utils;

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

    let config = config::load_from_yaml(config_path)
        .await
        .context(format!(
            "Failed to load the configuration file from {}",
            config_path
        ))
        .log_error()?;

    // Set up logging
    let _logger_guard = init_logging(&config)?;

    // Finally starting!
    info!("{} version {} started.", crate_name!(), crate_version!());

    // Database
    let db_opts = PgConnectOptions::new()
        .username(&config.database.username)
        .password(&config.database.password)
        .host(&config.database.host)
        .port(config.database.port)
        .database(&config.database.database);

    info!(
        "Connecting to database at {}:{}",
        &config.database.host, &config.database.port
    );
    let db_pool = PgPool::connect_with(db_opts)
        .await
        .context("Failed to connect to the Database")
        .log_error()?;

    // Database migration
    let _guard = span!(Level::INFO, "migrations").entered();
    info!("Running database migrations if needed...");
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&db_pool)
        .await
        .context("Failed migrating the database to the latest version")
        .log_error()?;
    info!("Migrations completed successfully");
    drop(_guard);

    // Inverter specifications loading
    if config.inverters_dir.is_none() {
        info!("No inverters path specified. Using default");
    }
    let json = fs::read_to_string(
        config
            .inverters_dir
            .unwrap_or("./inverters/Growatt v6.json".to_string()),
    )
    .await
    .context("Could not load inverters definitions")
    .log_error()?;

    let inverter: Arc<Vec<GrowattV6EnergyFragment>> =
        Arc::new(serde_json::from_str(&json).log_error()?);

    // Socket opening
    // https://github.com/mqudsi/tcpproxy/blob/master/src/main.rs
    let listen_port = config.listen_port.unwrap_or(5279);
    let listener = TcpListener::bind(format!("{}:{:?}", "0.0.0.0", listen_port))
        .await
        .with_context(|| format!("Failed to open port {:?}", listen_port))
        .log_error()?;

    info!(
        "Started listening for incoming connections on port {:?}",
        listen_port
    );

    // Listener Setup
    let _listener_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
        loop {
            let (client, client_addr) = listener.accept().await?;

            let i = inverter.clone();
            let pool = db_pool.clone();
            let addr = config.remote_address.clone();

            tokio::spawn(async move {
                let handler = server::Server::new(i, pool, addr);

                if let Err(e) = handler.handle_connection(client, client_addr).await {
                    error!(error = %e, "An error occurred while handling a connection from {}", client_addr);
                }
            });
        }
    });

    // Termination conditions
    let ctrl_c = async {
        signal::ctrl_c().await.unwrap();
    };

    let sigterm = async {
        signal::unix::signal(SignalKind::terminate())
            .unwrap()
            .recv()
            .await;
    };

    tokio::pin!(ctrl_c, sigterm);
    // Wait for a termination condition
    futures::future::select(ctrl_c, sigterm).await;

    info!("Received shutdown signal. Stopping.");

    Ok(())
}

fn init_logging(config: &Config) -> WorkerGuard {
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
        .arg(
            arg!(config_path: -c --config_path <PATH> "Path to configuration file")
                .help("Path to the config file to use to run the server")
                .default_value("./config.yaml"),
        )
}
