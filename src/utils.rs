use anyhow::anyhow;

pub fn unscramble_data(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let ndecdata = data.len();
    let mask = b"Growatt";

    // Start the decrypt routine
    // Isolate the already unscrambled header
    let mut unscrambled: Vec<u8> = match data.get(..8) {
        Some(u) => u.to_vec(),
        None => {
            return Err(anyhow!(
                "Data received too short: {} bytes instead of 8",
                data.len()
            ))
        }
    };

    for (i, j) in (8..ndecdata).zip((0..mask.len()).cycle()) {
        let dec_byte = data[i] ^ mask[j];
        unscrambled.push(dec_byte);
    }

    Ok(unscrambled)
}

pub fn hex_bytes_to_ascii(hex_bytes: &[u8]) -> String {
    hex_bytes.iter().map(|b| *b as char).collect()
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
