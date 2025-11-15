use freemdu::{Interface, serial::Port};
use std::{error::Error, thread, time::Duration};
use tokio::time;

const UNLOCK_TIMEOUT: Duration = Duration::from_millis(500);
const ERROR_RETRY_DELAY: Duration = Duration::from_secs(4);
const CHECK_TIMEOUT: Duration = Duration::from_millis(100);

async fn find_read_access_key(intf: &mut Interface<Port>) -> Result<u16, Box<dyn Error>> {
    for i in 0x0000..=0xffff {
        println!("Trying read access key: {i:04x}");

        while let Err(err) = time::timeout(UNLOCK_TIMEOUT, async {
            intf.query_software_id().await?;
            intf.unlock_read_access(i).await
        })
        .await
        {
            eprintln!("Error: {err}");
            thread::sleep(ERROR_RETRY_DELAY);
        }

        // Check if read access was successfully unlocked
        if let Ok(Ok(_)) = time::timeout(CHECK_TIMEOUT, intf.read_memory::<u8, _>(0x0000)).await {
            return Ok(i);
        }
    }

    Err("Failed to find read access key".into())
}

async fn find_full_access_key(
    intf: &mut Interface<Port>,
    read_key: u16,
) -> Result<u16, Box<dyn Error>> {
    for i in 0x0000..=0xffff {
        println!("Trying read & full access keys: {read_key:04x}, {i:04x}");

        while let Err(err) = time::timeout(UNLOCK_TIMEOUT, async {
            intf.query_software_id().await?;
            intf.unlock_read_access(read_key).await?;
            intf.unlock_full_access(i).await
        })
        .await
        {
            eprintln!("Error: {err}");
            thread::sleep(ERROR_RETRY_DELAY);
        }

        // Check if full access was successfully unlocked
        if let Ok(Ok(())) = time::timeout(CHECK_TIMEOUT, intf.halt()).await {
            return Ok(i);
        }
    }

    Err("Failed to find full access key".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let port = freemdu::serial::open("/dev/ttyACM0")?;
    let mut intf = Interface::new(port);
    let read_key = find_read_access_key(&mut intf).await?;
    let full_key = find_full_access_key(&mut intf, read_key).await?;

    println!("Found keys: {read_key:04x}, {full_key:04x}");

    Ok(())
}
