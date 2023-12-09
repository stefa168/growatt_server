use anyhow::{Context, Result};

pub fn unscramble_data(data: &[u8], mask: Option<&str>) -> Result<Vec<u8>> {
    let ndecdata = data.len();
    let mask: &[u8] = mask.unwrap_or("Growatt").as_bytes();

    // Start the decrypt routine
    // Isolate the already unscrambled header
    let mut unscrambled: Vec<u8> = data
        .get(..8)
        .with_context(|| format!("Data received too short: {} bytes instead of 8", data.len()))?
        .to_vec();

    for (i, j) in (8..ndecdata).zip((0..mask.len()).cycle()) {
        let dec_byte = data[i] ^ mask[j];
        unscrambled.push(dec_byte);
    }

    Ok(unscrambled)
}

pub fn hex_bytes_to_ascii(hex_bytes: &[u8]) -> String {
    hex_bytes.iter().map(|b| *b as char).collect()
}

pub fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let s = std::str::from_utf8(chunk).unwrap();
            u8::from_str_radix(s, 16).unwrap()
        })
        .collect()
}

#[allow(dead_code)]
fn print_bytes(bytes: &[u8], n: usize) {
    bytes.chunks(n).enumerate().for_each(|(i, chunk)| {
        if i != 0 {
            println!();
        }
        print!("{:04x}: ", i * n);
        chunk.iter().enumerate().for_each(|(j, byte)| {
            if j != 0 && j % (n / 2) == 0 {
                print!(" ");
            }
            print!("{:02x} ", byte);
        });
        print!("  ");
        chunk.iter().for_each(|byte| {
            print!("{}", *byte as char);
        });
    });
}

macro_rules! log_error {
    ($expr:expr) => {
        match $expr {
            Ok(val) => Ok(val),
            Err(err) => {
                tracing::error!("{:?}", err);
                Err(err)
            }
        }
    };
}
