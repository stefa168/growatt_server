use crate::data::Datatype;
use serde::{Deserialize, Serialize};

pub mod message_type;

#[derive(Serialize, Deserialize, Debug)]
pub struct GrowattV6EnergyFragment {
    pub name: String,
    pub offset: u32,
    #[serde(alias = "length")]
    pub bytes_len: u32,
    #[serde(alias = "type")]
    pub fragment_type: Datatype,
    pub fraction: Option<u32>,
}
