use std::collections::HashMap;
use std::sync::Arc;
use std::f32;
use crate::{Datatype, GrowattV6EnergyFragment, utils};

#[derive(Debug)]
pub struct Data4Message {
    header: Vec<u8>,
    data: HashMap<String, String>,
}

impl Data4Message {
    pub fn new(inverter_fragments: Arc<Vec<GrowattV6EnergyFragment>>, bytes: &Vec<u8>) -> Result<Self, String> {
        let bytes = bytes.clone();

        let header: Vec<u8> = bytes[0..=7].to_vec();

        let bytes = &bytes[8..];
        let mut data = HashMap::new();

        for fragment in inverter_fragments.iter() {
            let base_offset = fragment.offset as usize;
            let end_offset = base_offset + fragment.bytes_len as usize;

            let slice = &bytes[base_offset..end_offset];

            let string_value = match &fragment.fragment_type {
                Datatype::String => {
                    utils::hex_bytes_to_ascii(&slice)
                        .chars()
                        .filter(|c| c.is_alphanumeric()).collect::<String>()
                }
                Datatype::Date => {
                    println!("{}/{}/{} {}:{}:{}", slice[0], slice[1], slice[2], slice[3], slice[4], slice[5]);
                    let year = 2000 + <i32>::from(slice[0]);
                    let month = slice[1].into();
                    let day = slice[2].into();
                    let hour = slice[3].into();
                    let min = slice[4].into();
                    let sec = slice[5].into();
                    let date = chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap().and_hms_opt(hour, min, sec).unwrap();

                    date.to_string()
                }
                Datatype::Integer => {
                    let mut four_bytes = Vec::from(slice);

                    for _ in 0..(4 - four_bytes.len()) {
                        four_bytes.insert(0, 0);
                    }

                    let four_bytes: [u8; 4] = four_bytes.try_into().or_else(|e| {
                        eprintln!("Error converting slice to array: {:?}", e);
                        return Err(e);
                    }).unwrap();

                    let value = u32::from_be_bytes(four_bytes);

                    value.to_string()
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

                    ((value as f32) / (fragment.fraction.unwrap_or(1) as f32)).to_string()
                }
            };

            data.insert(fragment.name.clone(), string_value);
        }

        Ok(Self { header, data })
    }
}
