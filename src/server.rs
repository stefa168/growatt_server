use crate::data_message::{DataMessage, MessageType};
use crate::{utils, GrowattV6EnergyFragment, BUF_SIZE};
use anyhow::Context;
use futures::FutureExt;
use std::fmt::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument};

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

    #[instrument(skip(self), name = "message_handler")]
    async fn handle_data<'a>(&self, data: &'a [u8]) -> anyhow::Result<&'a [u8]> {
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
