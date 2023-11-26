use crate::{utils, Datatype, GrowattV6EnergyFragment};
use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32;
use std::sync::Arc;
use tracing::debug;

#[derive(Debug)]
pub struct DataMessage {
    pub raw: Vec<u8>,
    pub header: Vec<u8>,
    pub data_type: MessageType,
    pub data: HashMap<String, String>,
    pub time: DateTime<Local>,
    pub serial_number: Option<String>,
}

impl DataMessage {
    pub fn data4(
        inverter_fragments: Arc<Vec<GrowattV6EnergyFragment>>,
        bytes: &[u8],
    ) -> Result<Self> {
        let bytes = bytes.to_owned();

        let header: Vec<u8> = bytes[0..=7].to_vec();

        let bytes = &bytes[8..];
        let mut data = HashMap::new();

        let time = Local::now();
        let mut serial_number: Option<String> = None;

        for fragment in inverter_fragments.iter() {
            let base_offset = fragment.offset as usize;
            let end_offset = base_offset + fragment.bytes_len as usize;

            let slice = &bytes[base_offset..end_offset];

            let string_value = match &fragment.fragment_type {
                Datatype::String => {
                    let s = utils::hex_bytes_to_ascii(slice)
                        .chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect::<String>();

                    // todo make Serial Number identification less hardcoded
                    if &fragment.name == "Inverter SN" {
                        serial_number = Some(s.clone());
                    }

                    s
                }
                Datatype::Date => {
                    debug!(
                        "Got date type with value: {}/{}/{} {}:{}:{}",
                        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5]
                    );
                    let year = 2000 + <i32>::from(slice[0]);
                    let month = slice[1].into();
                    let day = slice[2].into();
                    let hour = slice[3].into();
                    let min = slice[4].into();
                    let sec = slice[5].into();
                    let date = chrono::NaiveDate::from_ymd_opt(year, month, day)
                        .unwrap()
                        .and_hms_opt(hour, min, sec)
                        .unwrap();

                    date.to_string()
                }
                Datatype::Integer => {
                    let mut four_bytes = Vec::from(slice);

                    for _ in 0..(4 - four_bytes.len()) {
                        four_bytes.insert(0, 0);
                    }

                    // todo log and continue with next fragment
                    let four_bytes: [u8; 4] = four_bytes
                        .try_into()
                        .map_err(|e| {
                            eprintln!("Error converting slice to array: {:?}", e);
                            e
                        })
                        .unwrap();

                    let value = u32::from_be_bytes(four_bytes);

                    value.to_string()
                }
                Datatype::Float => {
                    let mut four_bytes = Vec::from(slice);

                    for _ in 0..(4 - four_bytes.len()) {
                        four_bytes.insert(0, 0);
                    }

                    let four_bytes: [u8; 4] = four_bytes
                        .try_into()
                        .map_err(|e| {
                            eprintln!("Error converting slice to array: {:?}", e);
                            e
                        })
                        .unwrap();

                    let value = u32::from_be_bytes(four_bytes);

                    ((value as f32) / (fragment.fraction.unwrap_or(1) as f32)).to_string()
                }
            };

            data.insert(fragment.name.clone(), string_value);
        }

        Ok(Self {
            raw: bytes.into(),
            header,
            data_type: MessageType::Data4,
            data,
            time,
            serial_number,
        })
    }

    pub fn placeholder(bytes: &[u8], message_type: MessageType) -> Result<Self> {
        let bytes = bytes.to_owned();
        let header: Vec<u8> = bytes[0..=7].to_vec();

        let time = Local::now();

        Ok(Self {
            raw: bytes,
            header,
            data_type: message_type,
            data: Default::default(),
            time,
            serial_number: None,
        })
    }
}

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
pub enum MessageType {
    Data3,
    Data4,
    Ping,
    Configure,
    Identify,
    Unknown,
}
