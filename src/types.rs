#[derive(Debug)]
pub enum MessageType {
    DATA3,
    DATA4,
    PING,
    CONFIGURE,
    IDENTIFY,
    UNKNOWN(u8),
}
