use crate::config::Config;
use crate::data::v6::message_type::MessageType;
use crate::data::v6::GrowattV6EnergyFragment;
use crate::data_message::DataMessage;
use crate::log_ext::LogErrorExt;
use crate::{utils, BUF_SIZE};
use anyhow::Context;
use chrono::Local;
use futures::FutureExt;
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use std::fmt::Write;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::signal::unix::SignalKind;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::Level;
use tracing::{debug, error, info, instrument, span};
use server::Server;

mod server;

/// Main server loop. This listens for connections, and spawns tasks to handle them.
pub async fn run_server(
    config: Arc<Config>,
    inverter: Arc<Vec<GrowattV6EnergyFragment>>,
) -> anyhow::Result<()> {
    let config = config.clone();

    // Final setup phases
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
                let handler = Server::new(i, pool, addr);

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
