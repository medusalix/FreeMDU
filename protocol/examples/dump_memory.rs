use std::{
    error::Error,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut port = freemdu::serial::open("/dev/ttyACM0")?;
    let mut dev = freemdu::device::connect(&mut port).await?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("memory_dump.bin")?;

    // Resume dumping process if previously interrupted
    let start: u16 = file.seek(SeekFrom::End(0))?.try_into()?;

    for addr in (start..=0xffff).step_by(0x80) {
        println!("Reading memory address {addr:04x}");

        let data: [u8; 0x80] = dev.interface().read_memory(addr).await?;

        file.write_all(&data)?;
    }

    Ok(())
}
