use serde::{Deserialize, Serialize};

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
pub enum MessageType {
    DATA3,
    DATA4,
    PING,
    CONFIGURE,
    IDENTIFY,
    UNKNOWN,
}
