#![no_std]
#![no_main]

// Ez a sor regisztrálja a globális memóriafoglalót:
extern crate alloc;

use core::str::FromStr;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer, WithTimeout};
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

impl Write for OpticalPort<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let len = self.0.write_async(buf).await?;
        for _ in 0..len {
            self.read(&mut [0x00]).await?;
        }
        Ok(len)
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
#[allow(unused_variables)]
async fn main(spawner: Spawner) {
    logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 32 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    info!("Initializing UART1 test program...");

    let mut status_led = new_status_led();
    status_led.set_high();

    let mut port = match new_optical_port(peripherals.UART1) {
        Ok(p) => p,
        Err(err) => {
            error!("UART setup error: {err:?}");
            return;
        }
    };

    info!("UART1 initialized. Listening for CR+LF terminated lines...");

    let mut rx_buf = [0u8; 1];
    let mut line_buf = [0u8; 256];
    let mut line_len = 0;

    loop {
        match port
            .read(&mut rx_buf)
            .with_timeout(Duration::from_millis(100))
            .await
        {
            Ok(Ok(len)) if len > 0 => {
                let byte = rx_buf[0];

                info!("Rx byte: 0x{byte:02X}");

                if line_len < line_buf.len() {
                    line_buf[line_len] = byte;
                    line_len += 1;
                } else {
                    line_len = 0;
                }

                if line_len >= 2
                    && line_buf[line_len - 2] == b'\r'
                    && line_buf[line_len - 1] == b'\n'
                {
                    let text = core::str::from_utf8(&line_buf[..line_len - 2])
                        .unwrap_or("<invalid ASCII>");
                    info!("Received line: {text}");

                    status_led.set_low();
                    Timer::after(Duration::from_millis(100)).await;
                    status_led.set_high();
                    Timer::after(Duration::from_millis(100)).await;

                    line_len = 0;
                }
            }
            Ok(Err(err)) => {
                error!("UART read error: {err:?}");
            }
            _ => {}
        }
    }
}
