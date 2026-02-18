use clap::Parser;
use std::{
    error::Error,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Start address for EEPROM dump (hex format, e.g. `0x0000`)
    #[arg(short, long, value_parser = parse_hex, default_value = "0x0000")]
    start: u16,

    /// End address for EEPROM dump (hex format, e.g. `0x07ff`)
    #[arg(short, long, value_parser = parse_hex, default_value = "0x07ff")]
    end: u16,

    /// Use byte addressing instead of word addressing (for newer appliances)
    #[arg(short, long, default_value = "false")]
    byte_addressing: bool,

    /// Output filename for EEPROM dump
    #[arg(short, long, default_value = "eeprom_dump.bin")]
    output: String,

    /// Serial port path
    #[arg(short, long, default_value = "/dev/ttyACM0")]
    port: String,
}

fn parse_hex(s: &str) -> Result<u16, std::num::ParseIntError> {
    if let Some(stripped) = s.strip_prefix("0x") {
        u16::from_str_radix(stripped, 16)
    } else {
        u16::from_str_radix(s, 16)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args = Args::parse();

    let mut port = freemdu::serial::open(&args.port)?;
    let mut dev = freemdu::device::connect(&mut port).await?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.output)?;

    // Resume dumping process if previously interrupted
    let offset: u16 = file.seek(SeekFrom::End(0))?.try_into()?;

    for addr in (args.start + offset..=args.end).step_by(0x80) {
        println!("Reading EEPROM address {addr:04x}");

        // Convert address based on addressing mode
        let eeprom_addr = if args.byte_addressing {
            addr // Newer devices: use byte address directly
        } else {
            addr / 2 // Older devices: convert to word address
        };

        let data: [u8; 0x80] = dev.interface().read_eeprom(eeprom_addr).await?;

        file.write_all(&data)?;
    }

    Ok(())
}
