use clap::Parser;
use std::{
    error::Error,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Start address for memory dump (hex format, e.g. `0x0000_0000`)
    #[arg(short, long, value_parser = parse_hex, default_value = "0x0000_0000")]
    start: u32,

    /// End address for memory dump (hex format, e.g. `0x0000_ffff`)
    #[arg(short, long, value_parser = parse_hex, default_value = "0x0000_ffff")]
    end: u32,

    /// Output filename for memory dump
    #[arg(short, long, default_value = "memory_dump.bin")]
    output: String,

    /// Serial port path
    #[arg(short, long, default_value = "/dev/ttyACM0")]
    port: String,
}

fn parse_hex(s: &str) -> Result<u32, std::num::ParseIntError> {
    // Remove underscores that may be used for readability (e.g., 0x0000_ffff)
    let s = s.replace('_', "");

    if let Some(stripped) = s.strip_prefix("0x") {
        u32::from_str_radix(stripped, 16)
    } else {
        u32::from_str_radix(&s, 16)
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
    let offset: u32 = file.seek(SeekFrom::End(0))?.try_into()?;

    for addr in (args.start + offset..=args.end).step_by(0x80) {
        println!("Reading memory address {addr:08x}");

        let data: [u8; 0x80] = dev.interface().read_memory(addr).await?;

        file.write_all(&data)?;
    }

    Ok(())
}
