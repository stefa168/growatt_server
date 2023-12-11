use crate::config::Config;
use crate::data::v6::message_type::MessageType;
use crate::data::v6::GrowattV6EnergyFragment;
use crate::data_message::DataMessage;
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

pub(crate) struct Server {
    inverter: Arc<Vec<GrowattV6EnergyFragment>>,
    db_pool: sqlx::Pool<sqlx::Postgres>,
    remote_address: Option<String>,
}

impl Server {
    pub fn new(
        inverter: Arc<Vec<GrowattV6EnergyFragment>>,
        db_pool: sqlx::Pool<sqlx::Postgres>,
        remote_address: Option<String>,
    ) -> Self {
        Self {
            inverter,
            db_pool,
            remote_address,
        }
    }

    #[instrument(skip(self, data), name = "inverter_data_handler")]
    async fn handle_inverter_data<'a>(&self, data: &'a [u8]) -> anyhow::Result<&'a [u8]> {
        let bytes = utils::unscramble_data(data, None)?;

        let data_length = u16::from_be_bytes(bytes[4..6].try_into().unwrap());

        let message_type: MessageType = bytes[7].into();

        info!(
            "New {} message received from inverters, {} bytes long.",
            message_type, data_length
        );
        debug!(
            "Message data: {}",
            bytes.iter().fold(String::new(), |mut output, b| {
                write!(output, "{:02x}", b).unwrap();
                output
            })
        );

        let message = match message_type {
            MessageType::Data3 => DataMessage::placeholder(&bytes, MessageType::Data3),
            MessageType::Data4 => DataMessage::data4(self.inverter.clone(), &bytes),
            MessageType::Ping => DataMessage::placeholder(&bytes, MessageType::Ping),
            MessageType::Configure => DataMessage::placeholder(&bytes, MessageType::Configure),
            MessageType::Identify => DataMessage::placeholder(&bytes, MessageType::Identify),
            MessageType::Unknown => DataMessage::placeholder(&bytes, MessageType::Unknown),
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

    #[instrument(skip(self, data), name = "remote_data_handler")]
    async fn handle_remote_data<'a>(&self, data: &'a [u8]) -> anyhow::Result<&'a [u8]> {
        let bytes = utils::unscramble_data(data, None)?;
        let data_length = bytes.len();

        let time = Local::now();

        let r = sqlx::query!(
            "INSERT INTO remote_messages (raw, time) VALUES ($1, $2) RETURNING id",
            bytes,
            time
        )
        .fetch_one(&self.db_pool)
        .await;

        if let Err(e) = r {
            error!(error=%e);
            return Ok(data);
        }

        let id = r.unwrap().id;

        info!(
            "New message received from remote, {} bytes long. ID = {}",
            data_length - 8,
            id
        );
        debug!(
            "Message data: {}",
            bytes.iter().fold(String::new(), |mut output, b| {
                write!(output, "{:02x}", b).unwrap();
                output
            })
        );

        Ok(data)
    }

    async fn copy_with_abort<R, W>(
        &self,
        read: &mut R,
        write: &mut W,
        abort: CancellationToken,
        handle_data: bool,
    ) -> anyhow::Result<usize>
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
            let handling_result = match handle_data {
                false => self.handle_remote_data(&buf[..bytes_read]).await,
                true => self.handle_inverter_data(&buf[..bytes_read]).await,
            };

            let bytes_to_forward = match handling_result {
                Ok(d) => d,
                Err(e) => {
                    error!(error=%e, "An error occurred while processing a remote response packet");
                    continue 'proxy;
                }
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
    ) -> anyhow::Result<()> {
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
    let db_pool = log_error!(PgPool::connect_with(db_opts)
        .await
        .context("Failed to connect to the Database"))?;

    // Database migration
    let _guard = span!(Level::INFO, "migrations").entered();
    info!("Running database migrations if needed...");
    let migrator = sqlx::migrate!("./migrations");
    log_error!(migrator
        .run(&db_pool)
        .await
        .context("Failed migrating the database to the latest version"))?;
    info!("Migrations completed successfully");
    drop(_guard);

    // Socket opening
    // https://github.com/mqudsi/tcpproxy/blob/master/src/main.rs
    let listen_port = config.listen_port.unwrap_or(5279);
    let listener = log_error!(
        TcpListener::bind(format!("{}:{:?}", "0.0.0.0", listen_port))
            .await
            .with_context(|| format!("Failed to open port {:?}", listen_port))
    )?;

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
