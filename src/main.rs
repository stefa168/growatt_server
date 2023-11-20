use anyhow::{Context, Result};
use clap::{arg, crate_authors, crate_description, crate_name, crate_version, Command};
use data_message::DataMessage;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use std::fmt::Write;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::SignalKind;
use tokio::task::JoinHandle;
use tokio::{fs, signal};
use tokio_util::sync::CancellationToken;
use types::MessageType;

mod config;
mod data_message;
mod types;
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
async fn main() -> Result<()> {
    let args = get_cli_conf().get_matches();

    let config_path: &String = args.get_one("config_path").unwrap();

    let config = config::load_from_yaml(config_path)
        .await
        .context("Failed to load the configuration file")?;

    let db_opts = PgConnectOptions::new()
        .username(&config.database.username)
        .password(&config.database.password)
        .host(&config.database.host)
        .port(config.database.port)
        .database(&config.database.database);

    let db_pool = PgPool::connect_with(db_opts)
        .await
        .context("Failed to connect to the Database")?;

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
        .with_context(|| format!("Failed to open port {:?}", listen_port))?;

    println!(
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

                handler
                    .handle_connection(client, client_addr)
                    .await
                    .with_context(|| {
                        format!(
                            "An error occurred while handling a connection from {}",
                            client_addr
                        )
                    })

                /*                if let Err(e) = handler.handle_connection(client, client_addr).await {
                    eprintln!(
                        "An error occurred while handling a connection from {}: {}",
                        client_addr, e
                    );
                }*/
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

    println!("Received shutdown signal. Stopping.");

    Ok(())
}

struct ConnectionHandler {
    inverter: Arc<Vec<GrowattV6EnergyFragment>>,
    db_pool: sqlx::Pool<sqlx::Postgres>,
    remote_address: Option<String>,
}

impl ConnectionHandler {
    async fn handle_data<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let bytes = utils::unscramble_data(data);

        println!(
            "New message! {}",
            bytes.iter().fold(String::new(), |mut output, b| {
                write!(output, "{:02x}", b).unwrap();
                output
            })
        );

        let data_length = u16::from_be_bytes(bytes[4..6].try_into().unwrap());

        println!("Data length: {data_length} bytes");

        let message = match bytes[7] {
            0x03 => DataMessage::placeholder(&bytes, MessageType::Data3),
            0x04 => DataMessage::data4(self.inverter.clone(), &bytes),
            0x16 => DataMessage::placeholder(&bytes, MessageType::Ping),
            0x18 => DataMessage::placeholder(&bytes, MessageType::Configure),
            0x19 => DataMessage::placeholder(&bytes, MessageType::Identify),
            _ => DataMessage::placeholder(&bytes, MessageType::Unknown),
        };

        let datamessage = message.unwrap();

        println!("Message type: {:?}", &datamessage.data_type);

        let r = sqlx::query!("INSERT INTO inverter_messages (raw, type, header, time) VALUES ($1, $2, $3, $4) returning id",
            datamessage.raw, serde_json::to_string(&datamessage.data_type).unwrap(), datamessage.header, datamessage.time)
            .fetch_one(&self.db_pool)
            // todo handle unlikely scenarios
            .await;

        if let Err(e) = r {
            println!("{}", e);
            return data;
        }

        let id = r.unwrap().id;

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

        data
    }

    async fn copy_with_abort<R, W>(
        &self,
        read: &mut R,
        write: &mut W,
        abort: CancellationToken,
        handle_data: bool,
    ) -> tokio::io::Result<usize>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut bytes_forwarded = 0;
        let mut buf = [0u8; BUF_SIZE];

        loop {
            let bytes_read;
            tokio::select! {
                biased;

                result = read.read(&mut buf) => {
                    bytes_read = result?;
                },
                _ = abort.cancelled() => {
                    break;
                }
            }

            if bytes_read == 0 {
                break;
            }

            let bytes_to_forward = match handle_data {
                false => &buf[..bytes_read],
                true => self.handle_data(&buf[..bytes_read]).await,
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
        println!("New connection from {}", client_addr);

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

        match client_copied {
            Ok(count) => {
                eprintln!(
                    "Transferred {} bytes from proxy client {} to upstream server",
                    count, client_addr
                );
            }
            Err(err) => {
                eprintln!(
                    "Error writing bytes from proxy client {} to upstream server",
                    client_addr
                );
                eprintln!("{}", err);
            }
        };

        match remote_copied {
            Ok(count) => {
                eprintln!(
                    "Transferred {} bytes from upstream server to proxy client {}",
                    count, client_addr
                );
            }
            Err(err) => {
                eprintln!(
                    "Error writing bytes from upstream server to proxy client {}",
                    client_addr
                );
                eprintln!("{}", err);
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
