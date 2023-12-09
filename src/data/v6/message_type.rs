use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
pub enum MessageType {
    Data3,
    Data4,
    Ping,
    Configure,
    Identify,
    Unknown,
}

// implement from for MessageTyoe
impl From<u8> for MessageType {
    fn from(byte: u8) -> Self {
        match byte {
            0x03 => MessageType::Data3,
            0x04 => MessageType::Data4,
            0x16 => MessageType::Ping,
            0x18 => MessageType::Configure,
            0x19 => MessageType::Identify,
            _ => MessageType::Unknown,
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MessageType::Data3 => write!(f, "Data3"),
            MessageType::Data4 => write!(f, "Data4"),
            MessageType::Ping => write!(f, "Ping"),
            MessageType::Configure => write!(f, "Configure"),
            MessageType::Identify => write!(f, "Identify"),
            MessageType::Unknown => write!(f, "Unknown"),
        }
    }
}
