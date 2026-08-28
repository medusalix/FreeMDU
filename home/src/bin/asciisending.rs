#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use core::str::FromStr;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_io_async::{ErrorType, Read, ReadExactError, Write};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    Async,
    gpio::{AnyPin, Input, InputConfig, Level, Output, OutputConfig},
    interrupt::software::SoftwareInterruptControl,
    timer::timg::TimerGroup,
    uart::{Config, ConfigError, Instance, IoError, Parity, Uart},
};
use esp_println::logger;
use log::{error, info};

esp_bootloader_esp_idf::esp_app_desc!();

#[macro_export]
macro_rules! num_from_env {
    ($name:literal, $type:ty) => {
        match <$type>::from_str_radix(env!($name), 10) {
            Ok(val) => val,
            Err(_) => panic!("failed to parse environment variable as number"),
        }
    };
}

pub struct OpticalPort<'a>(Uart<'a, Async>);

impl ErrorType for OpticalPort<'_> {
    type Error = IoError;
}

impl Read for OpticalPort<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            if let Ok(len) = self.0.read_async(buf).await {
                return Ok(len);
            }
        }
    }

    async fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<(), ReadExactError<Self::Error>> {
        while !buf.is_empty() {
            let len = self.read(buf).await?;
            buf = &mut buf[len..];
        }
        Ok(())
    }
}

use embassy_time::WithTimeout;

impl Write for OpticalPort<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        // 1. Send the entire buffer at once (handled by hardware, no pause between bytes)
        // 1. Kiküldjük a teljes puffert egyszerre (a hardver intézi, nem lesz szünet a bájtok között)
        self.0.write_async(buf).await?;

        // 2. If you want to flush the RX buffer due to optical reflection, do so after sending the full line, not byte-by-byte:
        // 2. Ha az optikai tükröződés miatt ki akarod üríteni a vételi puffert, azt a teljes sor kiküldése után teszed meg, nem bájtonként:
        let mut dummy = [0u8; 1];
        while let Ok(Ok(len)) = self
            .read(&mut dummy)
            .with_timeout(Duration::from_millis(2))
            .await
        {
            if len == 0 {
                break;
            }
        }

        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(self.0.flush_async().await?)
    }
}

pub fn new_optical_port<'a>(uart: impl Instance + 'a) -> Result<OpticalPort<'a>, ConfigError> {
    const PIN_RX: u8 = num_from_env!("PIN_OPTICAL_RX", u8);
    const PIN_TX: u8 = num_from_env!("PIN_OPTICAL_TX", u8);
    let rx = Input::new(unsafe { AnyPin::steal(PIN_RX) }, InputConfig::default());
    let tx = Output::new(
        unsafe { AnyPin::steal(PIN_TX) },
        Level::Low,
        OutputConfig::default(),
    );
    let cfg = Config::default()
        .with_baudrate(2400)
        .with_parity(Parity::Even);
    let uart = Uart::new(uart, cfg)?
        .with_rx(rx.peripheral_input().with_input_inverter(true))
        .with_tx(tx.into_peripheral_output().with_output_inverter(true))
        .into_async();

    Ok(OpticalPort(uart))
}

#[must_use]
pub fn new_status_led<'a>() -> Output<'a> {
    let pin_num: u8 = option_env!("PIN_LED_STATUS")
        .and_then(|s| u8::from_str(s).ok())
        .unwrap_or(10);

    let led = unsafe { AnyPin::steal(pin_num) };

    Output::new(led, Level::High, OutputConfig::default())
}

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 32 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    info!("Initializing UART1 Transmitter...");

    let mut status_led = new_status_led();
    status_led.set_high();

    let mut port = match new_optical_port(peripherals.UART1) {
        Ok(p) => p,
        Err(err) => {
            error!("UART setup error: {err:?}");
            return;
        }
    };

    // --- SETUP SECTION ---
    // Serial.println("ASCII Table ~ Character Map");
    let _ = port.write(b"ASCII Table ~ Character Map\r\n").await;
    let _ = port.flush().await;

    info!("Started sending ASCII Table...");

    // int thisByte = 33;
    let mut this_byte: u8 = 33;

    // --- LOOP SECTION ---
    loop {
        let line = format!(
            "{}, dec: {}, hex: {:X}, oct: {:o}, bin: {:b}\r\n",
            this_byte as char, this_byte, this_byte, this_byte, this_byte
        );

        // Send with byte-by-byte reflection suppression
        // Küldés bájtonkénti tükröződés-elnyeléssel
        if let Err(e) = port.write(line.as_bytes()).await {
            error!("Write error: {e:?}");
        } else {
            let _ = port.flush().await;
        }

        // Status LED flash on send (Active Low)
        // Status LED felvillanása küldéskor (Active Low)
        status_led.set_low();
        Timer::after(Duration::from_millis(50)).await;
        status_led.set_high();

        // Cycle: between 33..=126
        // Léptetés: 33..=126 között
        if this_byte == 126 {
            this_byte = 33;
        } else {
            this_byte += 1;
        }

        // delay(1000);
        Timer::after(Duration::from_millis(1000)).await;
    }
}
