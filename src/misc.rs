use crate::config::Config;
use crate::data::v6::message_type::MessageType;
use crate::data::v6::GrowattV6EnergyFragment;
use crate::data_message::DataMessage;
use crate::utils;
use crate::utils::hex_to_bytes;
use anyhow::Context;
use clap::ArgMatches;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DecMessage {
    decrypt: Option<bool>,
    raw: String,
}

///
/// This function is one of the available subcommands of the CLI.
/// In particular, this one allows the user to decrypt a series of messages from a specified file.
///
pub(crate) async fn run_decrypt(
    args: &ArgMatches,
    _config: Arc<Config>,
    inverter: Arc<Vec<GrowattV6EnergyFragment>>,
) -> anyhow::Result<()> {
    let file_path: &String = args.get_one("file").unwrap();

    async fn load_from_json(path: &str) -> anyhow::Result<Vec<DecMessage>> {
        let json = fs::read_to_string(path).await?;
        let config = serde_json::from_str(&json)?;
        Ok(config)
    }

    let messages = load_from_json(file_path)
        .await
        .context("Could not load messages")?;

    for message in messages {
        let bytes = if message.decrypt.unwrap_or(false) {
            utils::unscramble_data(&hex_to_bytes(&message.raw), None)?
        } else {
            hex_to_bytes(&message.raw)
        };
        let _data_length = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
        let message_type: MessageType = bytes[7].into();

        let dm = match message_type {
            MessageType::Data3 => DataMessage::placeholder(&bytes, MessageType::Data3),
            MessageType::Data4 => DataMessage::data4(inverter.clone(), &bytes),
            MessageType::Ping => DataMessage::placeholder(&bytes, MessageType::Ping),
            MessageType::Configure => DataMessage::placeholder(&bytes, MessageType::Configure),
            MessageType::Identify => DataMessage::placeholder(&bytes, MessageType::Identify),
            MessageType::Unknown => DataMessage::placeholder(&bytes, MessageType::Unknown),
        }
        .unwrap();

        info!("{}", dm);
    }

    Ok(())
}
