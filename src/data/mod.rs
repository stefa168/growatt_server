use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Datatype {
    String,
    Date,
    #[serde(alias = "int")]
    Integer,
    Float,
}

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
