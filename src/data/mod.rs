pub mod v6;

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
