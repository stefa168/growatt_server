use crate::config::Config;
use anyhow::{Context, Result};
use clap::{arg, crate_authors, crate_description, crate_name, crate_version, Command};
use data_message::DataMessage;
use data_message::MessageType;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use std::fmt::{Debug, Write};
use std::io;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::SignalKind;
use tokio::task::JoinHandle;
use tokio::{fs, signal};
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, instrument, span, Level};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_unwrap::ResultExt;

mod config;
mod data_message;
mod utils;

const BUF_SIZE: usize = 65535;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum Datatype {
    String,
    Date,
    #[serde(alias = "int")]
    Integer,
    Float,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GrowattV6EnergyFragment {
    name: String,
    offset: u32,
    #[serde(alias = "length")]
    bytes_len: u32,
    #[serde(alias = "type")]
    fragment_type: Datatype,
    fraction: Option<u32>,
}

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

    let config = config::load_from_yaml(config_path).await.context(format!(
        "Failed to load the configuration file from {}",
        config_path
    ))?;

    // Set up logging
    let _logger_guard = init_logging(&config);

    // Finally starting!
    info!("{} version {} started.", crate_name!(), crate_version!());

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
        .expect_or_log("Failed to connect to the Database");

    let _guard = span!(Level::INFO, "migrations").entered();
    info!("Running database migrations if needed...");
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&db_pool)
        .await
        .expect_or_log("Failed migrating the database to the latest version");
    info!("Migrations completed successfully");
    drop(_guard);

    if config.inverters_dir.is_none() {
        info!("No inverters path specified. Using default");
    }
    let json = fs::read_to_string(
        config
            .inverters_dir
            .unwrap_or("./inverters/Growatt v6.json".to_string()),
    )
    .await?;
    let inverter: Arc<Vec<GrowattV6EnergyFragment>> = Arc::new(serde_json::from_str(&json)?);

    // https://github.com/mqudsi/tcpproxy/blob/master/src/main.rs
    let listen_port = config.listen_port.unwrap_or(5279);
    let listener = TcpListener::bind(format!("{}:{:?}", "0.0.0.0", listen_port))
        .await
        .expect_or_log(format!("Failed to open port {:?}", listen_port).as_str());

    info!(
        "Started listening for incoming connections on port {:?}",
        listen_port
    );

    let _listener_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
        loop {
            let (client, client_addr) = listener.accept().await?;

            let i = inverter.clone();
            let pool = db_pool.clone();
            let addr = config.remote_address.clone();

            tokio::spawn(async move {
                let handler = ConnectionHandler {
                    inverter: i,
                    db_pool: pool,
                    remote_address: addr,
                };

                if let Err(e) = handler.handle_connection(client, client_addr).await {
                    error!(error = %e, "An error occurred while handling a connection from {}", client_addr);
                }
            });
        }
    });

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
    futures::future::select(ctrl_c, sigterm).await;

    info!("Received shutdown signal. Stopping.");

    Ok(())
}

fn init_logging(config: &Config) -> WorkerGuard {
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

    let file_appender = tracing_appender::rolling::daily(
        options
            .and_then(|l| l.directory.clone())
            .unwrap_or("./logs".to_string()),
        "growatt_server",
    );
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(console_logging_filter)
        .with(fmt::layer().with_writer(non_blocking))
        .init();

    // Hook to log also panics with tracing
    std::panic::set_hook(Box::new(panic_hook));

    guard
}

struct ConnectionHandler {
    inverter: Arc<Vec<GrowattV6EnergyFragment>>,
    db_pool: sqlx::Pool<sqlx::Postgres>,
    remote_address: Option<String>,
}

impl ConnectionHandler {
    #[instrument(skip(self), name = "message_handler")]
    async fn handle_data<'a>(&self, data: &'a [u8]) -> Result<&'a [u8]> {
        let bytes = utils::unscramble_data(data)?;

        let data_length = u16::from_be_bytes(bytes[4..6].try_into().unwrap());

        fn byte_to_type(b: u8) -> String {
            match b {
                0x03 => "Data3".to_string(),
                0x04 => "Data4".to_string(),
                0x16 => "Ping".to_string(),
                0x18 => "Configure".to_string(),
                0x19 => "Identify".to_string(),
                v => format!("Unknown ({})", v),
            }
        }

        info!(
            "New {} message received, {} bytes long.",
            byte_to_type(bytes[7]),
            data_length
        );
        debug!(
            "Message data: {}",
            bytes.iter().fold(String::new(), |mut output, b| {
                write!(output, "{:02x}", b).unwrap();
                output
            })
        );

        let message = match bytes[7] {
            0x03 => DataMessage::placeholder(&bytes, MessageType::Data3),
            0x04 => DataMessage::data4(self.inverter.clone(), &bytes),
            0x16 => DataMessage::placeholder(&bytes, MessageType::Ping),
            0x18 => DataMessage::placeholder(&bytes, MessageType::Configure),
            0x19 => DataMessage::placeholder(&bytes, MessageType::Identify),
            _ => DataMessage::placeholder(&bytes, MessageType::Unknown),
        };

        let datamessage = message.unwrap();

        debug!("Message type: {:?}", &datamessage.data_type);

        // First save the complete message
        let r = sqlx::query!("INSERT INTO inverter_messages (raw, type, header, time, inverter_sn) VALUES ($1, $2, $3, $4, $5) returning id",
            datamessage.raw, serde_json::to_string(&datamessage.data_type).unwrap(), datamessage.header, datamessage.time, datamessage.serial_number)
            .fetch_one(&self.db_pool)
            // todo handle unlikely scenarios
            .await;

        if let Err(e) = r {
            error!(error=%e);
            return Ok(data);
        }

        let id = r.unwrap().id;

        // Then all the additional deserialized parts of the message (if present)
        for (key, value) in datamessage.data {
            sqlx::query!(
                "INSERT INTO message_data (message_id, key, value) VALUES ($1, $2, $3)",
                id,
                key,
                value
            )
            .execute(&self.db_pool)
            .await
            .unwrap();
        }

        Ok(data)
    }

    async fn copy_with_abort<R, W>(
        &self,
        read: &mut R,
        write: &mut W,
        abort: CancellationToken,
        handle_data: bool,
    ) -> Result<usize>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut bytes_forwarded = 0;
        let mut buf = [0u8; BUF_SIZE];

        'proxy: loop {
            let bytes_read;
            tokio::select! {
                biased;

                result = read.read(&mut buf) => {
                    bytes_read = result?;
                },
                _ = abort.cancelled() => {
                    break 'proxy;
                }
            }

            if bytes_read == 0 {
                break 'proxy;
            }

            /*
            Here the data is expected to be changed if it was requested to remove commands that
            the user doesn't want.
            */
            let bytes_to_forward = match handle_data {
                false => &buf[..bytes_read],
                true => match self.handle_data(&buf[..bytes_read]).await {
                    Ok(d) => d,
                    Err(e) => {
                        error!(error=%e, "An error occurred while processing a message packet");
                        continue 'proxy;
                    }
                },
            };

            write.write_all(bytes_to_forward).await?;
            bytes_forwarded += bytes_read;
        }

        Ok(bytes_forwarded)
    }

    pub async fn handle_connection(
        &self,
        mut client_stream: TcpStream,
        client_addr: SocketAddr,
    ) -> Result<()> {
        info!("New connection from {}", client_addr);

        let mut remote_server = TcpStream::connect(
            self.remote_address
                .as_ref()
                .unwrap_or(&"server.growatt.com:5279".to_string()),
        )
        .await
        .context("Error establishing remote connection")?;

        let (mut client_read, mut client_write) = client_stream.split();
        let (mut remote_read, mut remote_write) = remote_server.split();

        let cancellation_token = CancellationToken::new();

        let c3 = cancellation_token.clone();

        // add a wrapping tokio::select! to the tokio join in order to wait for ctrl_c
        // signal::ctrl_c().await?;
        let (remote_copied, client_copied) = tokio::join! {
            self.copy_with_abort(&mut remote_read, &mut client_write, cancellation_token.clone(), false).then(|r| {
                c3.cancel(); async {r}
            }),
            self.copy_with_abort(&mut client_read, &mut remote_write, cancellation_token.clone(), true).then(|r| {
                c3.cancel(); async {r}
            })
        };

        // Actions to be done after the connection has been closed by either of the two peers
        // (local or remote)
        match client_copied {
            Ok(count) => {
                info!(
                    "Transferred {} bytes from proxy client {} to upstream server",
                    count, client_addr
                );
            }
            Err(err) => {
                error!(
                    error=%err,
                    "Error writing bytes from proxy client {} to upstream server",
                    client_addr
                );
            }
        };

        match remote_copied {
            Ok(count) => {
                info!(
                    "Transferred {} bytes from upstream server to proxy client {}",
                    count, client_addr
                );
            }
            Err(err) => {
                error!(
                    error=%err,
                    "Error writing bytes from upstream server to proxy client {}",
                    client_addr
                );
            }
        };

        Ok(())
    }
}

fn get_cli_conf() -> Command {
    Command::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        // .arg_required_else_help(true)
        .arg(
            arg!(config_path: -c --config_path <PATH> "Path to configuration file")
                .help("Path to the config file to use to run the server")
                .default_value("./config.yaml"),
        )
}
