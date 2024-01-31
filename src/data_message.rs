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
use tracing::{debug, warn};

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

    pub fn meter_data(original_bytes: &[u8]) -> Result<Self> {
        let bytes = original_bytes.to_owned();

        let header: Vec<u8> = bytes[0..=7].to_vec();

        // Slice off the header and the CRC.
        let bytes = &bytes[8..(bytes.len() - 2)];
        let mut data = HashMap::new();

        let time = Local::now();
        let serial_number = Some(
            utils::hex_bytes_to_ascii(&bytes[0..30])
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>(),
        );

        // Skip the following 10 bytes, they currently don't have data we can use.
        // todo discover what those bytes are for
        //  They might be useful probably to understand what meter is being used from the ones compatible.
        let bytes = &bytes[40..bytes.len()];

        // todo move to configuration file to allow different meters to be used.
        const METER_DATA_KEYS: [&str; 40] = [
            "active_energy",
            "reactive_energy",
            "active_power_l1",
            "active_power_l2",
            "active_power_l3",
            "reactive_power_l1",
            "reactive_power_l2",
            "reactive_power_l3",
            "apparent_power_l1",
            "apparent_power_l2",
            "apparent_power_l3",
            "power_factor_l1",
            "power_factor_l2",
            "power_factor_l3",
            "voltage_l1",
            "voltage_l2",
            "voltage_l3",
            "current_l1",
            "current_l2",
            "current_l3",
            "active_power",
            "reactive_power",
            "apparent_power",
            "power_factor",
            "frequency",
            "posi_active_power",
            "reverse_active_power",
            "posi_reactive_power",
            "reverse_reactive_power",
            "apparent_energy",
            "total_active_energy_l1",
            "total_active_energy_l2",
            "total_active_energy_l3",
            "total_reactive_energy_l1",
            "total_reactive_energy_l2",
            "total_reactive_energy_l3",
            "total_energy",
            "l1_voltage_2",
            "l2_voltage_3",
            "l3_voltage_1",
        ];

        let values = utils::hex_bytes_to_ascii(bytes)
            .split(',')
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();

        for (i, value) in values.iter().enumerate() {
            let key = match METER_DATA_KEYS.get(i) {
                Some(k) => k.to_string(),
                None => {
                    warn!("Mismatch between number of keys {} and values {} in the received meter data.", METER_DATA_KEYS.len(), values.len());
                    break;
                }
            };

            data.insert(key, value.to_owned());
        }

        Ok(Self {
            raw: original_bytes.into(),
            header,
            data_type: MessageType::MeterData,
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

#[cfg(test)]
mod tests {
    use crate::data::v6::message_type::MessageType;
    use crate::data_message::DataMessage;
    use crate::utils::hex_to_bytes;

    #[test]
    fn test_meter_data() {
        let data = hex_to_bytes("00010006011C221B4458443333333333333300000000000000000000000000000000000000000043170C140A370C00F232313535342E32392C333133362E32342C3435362E342C3533342E322C3933352E372C34312E302C2D33372E372C2D3131392E362C3435382E322C3533352E352C3934332E332C302E392C302E392C302E392C3233362E392C3233322E392C3233352E332C342E312C342E352C352E342C313931382E362C2D37342E372C2D37342E372C302E392C35302E302C313236312E362C32303239322E362C3632382E392C323530372E322C32313738312E322C3732312E382C3737322E392C313634312E332C3732312E382C3737322E392C313634312E332C32343639302E35342C3430362E372C3430382E302C3430362E302C3481");

        let dm = DataMessage::meter_data(&data).unwrap();

        assert_eq!(dm.data_type, MessageType::MeterData);
        assert_eq!(dm.data.len(), 40);
        assert_eq!(dm.serial_number, Some("DXD3333333".to_string()));
        assert_eq!(
            dm.data.get("power_factor"),
            Some("0.9".to_string()).as_ref()
        );
    }
}
