use crate::data::v6::message_type::MessageType;
use crate::data::v6::GrowattV6EnergyFragment;
use crate::data::Datatype;
use crate::utils;
use anyhow::Result;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::f32;
use std::fmt::{Debug, Display, Write};
use std::sync::Arc;
use tracing::debug;

pub struct DataMessage {
    pub raw: Vec<u8>,
    pub header: Vec<u8>,
    pub data_type: MessageType,
    pub data: HashMap<String, String>,
    pub time: DateTime<Local>,
    pub serial_number: Option<String>,
}

fn _mark_usage(tracker: &mut HashMap<u32, String>, fragment: &GrowattV6EnergyFragment) {
    for i in 0..fragment.bytes_len {
        tracker.insert(fragment.offset + i, fragment.name.clone());
    }
}

fn _display_sequence(byte_sequence: &[u8], tracker: &HashMap<u32, String>) {
    let mut last_fragment = String::new();

    for (i, &byte) in byte_sequence.iter().enumerate() {
        match tracker.get(&(i as u32)) {
            Some(fragment_name) => {
                if fragment_name != &last_fragment {
                    if !last_fragment.is_empty() {
                        println!(); // Add an extra newline for separation
                    }
                    println!("\n{}", fragment_name);
                    last_fragment = fragment_name.clone();
                }
                print!("{:02x} ", byte);
            }
            None => {
                // This is a byte that does not belong to any fragment
                if !last_fragment.is_empty() {
                    println!("\nUNASSIGNED");
                    last_fragment.clear();
                }
                print!("{:02x} ", byte);
            }
        }
    }

    println!("\n");
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

        // let mut tracker = HashMap::new();

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

            //mark_usage(&mut tracker, fragment);

            debug!(
                "{:20} {:4} {:4} {}",
                fragment.name, fragment.offset, fragment.bytes_len, string_value
            );

            data.insert(fragment.name.clone(), string_value);
        }

        // display_sequence(&bytes, &tracker);

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

impl Display for DataMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut output = String::new();

        writeln!(output, "Data Message:")?;
        writeln!(output, "  Type: {}", self.data_type)?;
        writeln!(output, "  Time: {}", self.time)?;
        writeln!(output, "  Serial Number: {:?}", self.serial_number)?;
        writeln!(output, "  Data:")?;

        for (key, value) in self.data.iter() {
            writeln!(output, "    {}: {}", key, value)?;
        }

        write!(f, "{}", output)
    }
}

impl Debug for DataMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut output = String::new();

        writeln!(output, "Data Message:")?;
        writeln!(output, "  Raw: {:?}", self.raw)?;
        writeln!(output, "  Type: {}", self.data_type)?;
        writeln!(output, "  Time: {}", self.time)?;
        writeln!(output, "  Serial Number: {:?}", self.serial_number)?;
        writeln!(output, "  Data:")?;

        for (key, value) in self.data.iter() {
            writeln!(output, "    {}: {}", key, value)?;
        }

        write!(f, "{}", output)
    }
}
