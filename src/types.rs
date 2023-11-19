use serde::{Deserialize, Serialize};

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
pub enum MessageType {
    Data3,
    Data4,
    Ping,
    Configure,
    Identify,
    Unknown,
}
