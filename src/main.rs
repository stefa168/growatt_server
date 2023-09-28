use std::error::Error;
use std::{f32, io};
use std::io::{BufRead};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::{Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use tokio::{fs, signal};
use tokio::task::{JoinHandle};
use tokio_util::sync::CancellationToken;
use types::MessageType;

mod types;

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
async fn main() -> Result<(), Box<dyn Error>> {
    let json = fs::read_to_string("./inverters/Growatt v6.json").await?;
    let inverter: Arc<Vec<GrowattV6EnergyFragment>> = Arc::new(serde_json::from_str(&json)?);

    // https://github.com/mqudsi/tcpproxy/blob/master/src/main.rs
    let listener = TcpListener::bind("0.0.0.0:5279").await?;
    println!("Listening on {}", listener.local_addr().unwrap());

    let _listener_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
        loop {
            let (client, client_addr) = listener.accept().await?;

            let i = inverter.clone();
            tokio::spawn(async move {
                let handler = ConnectionHandler { inverter: i };
                if let Err(e) = handler.handle_connection(client, client_addr).await {
                    eprintln!("An error occurred while handling a connection from {}: {}", client_addr, e);
                }
            });
        };
    });

    if let Err(err) = signal::ctrl_c().await {
        eprintln!("Unable to listen for shutdown signal: {}", err);
        return Err(Box::try_from(err).unwrap());
        // we also shut down in case of error
    } else {
        println!("Received shutdown signal. Stopping.");
    }

    Ok(())
}

struct ConnectionHandler {
    inverter: Arc<Vec<GrowattV6EnergyFragment>>,
}

impl ConnectionHandler {
    async fn copy_with_abort<R, W>(
        &self,
        read: &mut R,
        write: &mut W,
        abort: CancellationToken,
        handle_data: bool) -> tokio::io::Result<usize>
        where R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin {
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
                true => self.handle_data(&buf[..bytes_read]),
            };

            write.write_all(bytes_to_forward).await?;
            bytes_forwarded += bytes_read;
        }

        Ok(bytes_forwarded)
    }

    fn handle_data<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let bytes = unscramble_data(data);

        println!("New message! {}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());

        let data_length = u16::from_be_bytes(bytes[4..6].try_into().unwrap());

        println!("Data length: {data_length} bytes");

        let message_type = match bytes[7] {
            0x03 => MessageType::DATA3,
            0x04 => MessageType::DATA4,
            0x16 => MessageType::PING,
            0x18 => MessageType::CONFIGURE,
            0x19 => MessageType::IDENTIFY,
            v => MessageType::UNKNOWN(v),
        };
        println!("Message type: {:?}", message_type);

        if matches!(message_type, MessageType::DATA4) {
            let bytes = &bytes[8..];

            for fragment in self.inverter.iter() {
                let base_offset = fragment.offset as usize;
                let end_offset = (base_offset + fragment.bytes_len as usize) as usize;

                let slice = &bytes[base_offset..end_offset];

                match fragment.fragment_type {
                    Datatype::String => {
                        println!("{}: {}",
                                 fragment.name,
                                 hex_bytes_to_ascii(&slice)
                                     .chars()
                                     .filter(|c| c.is_alphanumeric()).collect::<String>()
                        );
                    }
                    Datatype::Date => {
                        if slice.len() < 6 {
                            eprintln!("Not enough values for date! {}/6", slice.len());
                            continue;
                        }
                        println!("{}/{}/{} {}:{}:{}", slice[0], slice[1], slice[2], slice[3], slice[4], slice[5]);
                    }
                    Datatype::Integer/* | Datatype::Float*/ => {
                        let mut four_bytes = Vec::from(slice);

                        for _ in 0..(4 - four_bytes.len()) {
                            four_bytes.insert(0, 0);
                        }

                        let four_bytes: [u8; 4] = four_bytes.try_into().or_else(|e| {
                            eprintln!("Error converting slice to array: {:?}", e);
                            return Err(e);
                        }).unwrap();

                        println!("{}: {}", fragment.name, u32::from_be_bytes(four_bytes));
                    }
                    Datatype::Float => {
                        let mut four_bytes = Vec::from(slice);

                        for _ in 0..(4 - four_bytes.len()) {
                            four_bytes.insert(0, 0);
                        }

                        let four_bytes: [u8; 4] = four_bytes.try_into().or_else(|e| {
                            eprintln!("Error converting slice to array: {:?}", e);
                            return Err(e);
                        }).unwrap();

                        let value = u32::from_be_bytes(four_bytes);

                        println!("{}: {}", fragment.name, (value as f32) / (fragment.fraction.unwrap_or(1) as f32));
                    }
                }
            }
        }

        data
    }

    pub async fn handle_connection(&self, mut client_stream: TcpStream, client_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        println!("New connection from {}", client_addr);

        let mut remote_server = match TcpStream::connect("server.growatt.com:5279").await {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error establishing connection: {e}");
                return Err(Box::new(e));
            }
        };

        let (mut client_read, mut client_write) = client_stream.split();
        let (mut remote_read, mut remote_write) = remote_server.split();

        let cancellation_token = CancellationToken::new();

        let c3 = cancellation_token.clone();

        // add a wrapping tokio::select! to the tokio join in order to wait for ctrl_c
        // signal::ctrl_c().await?;
        let (remote_copied, client_copied) = tokio::join! {
                self.copy_with_abort(&mut remote_read, &mut client_write, cancellation_token.clone(), false).then(|r| {
                    let _ = c3.cancel(); async {r}
                }),
                self.copy_with_abort(&mut client_read, &mut remote_write, cancellation_token.clone(), true).then(|r| {
                    let _ = c3.cancel(); async {r}
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

fn unscramble_data(data: &[u8]) -> Vec<u8> {
    let ndecdata = data.len();
    let mask = b"Growatt";

    // Start the decrypt routine
    let mut unscrambled: Vec<u8> = data[..8].to_vec(); // Isolate the unscrambled header

    for (i, j) in (8..ndecdata).zip((0..mask.len()).cycle()) {
        let dec_byte = data[i] ^ mask[j];
        unscrambled.push(dec_byte);
    }

    unscrambled
}

fn hex_bytes_to_ascii(hex_bytes: &[u8]) -> String {
    hex_bytes.iter().map(|b| b.clone() as char).collect()
}

fn print_bytes(bytes: &[u8], n: usize) {
    bytes.chunks(n).enumerate().for_each(|(i, chunk)| {
        if i != 0 {
            println!();
        }
        print!("{:04x}: ", i * n);
        chunk.iter().enumerate().for_each(|(j, byte)| {
            if j != 0 && j % (n / 2) == 0 {
                print!(" ");
            }
            print!("{:02x} ", byte);
        });
        print!("  ");
        chunk.iter().for_each(|byte| {
            print!("{}", *byte as char);
        });
    });
}
