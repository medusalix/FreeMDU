#![no_std]
#![no_main]

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
async fn main(_spawner: Spawner) {
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

    info!("UART1 initialized. Validating sequence & consistency...");

    let mut rx_buf = [0u8; 1];
    let mut line_buf = [0u8; 256];
    let mut line_len = 0;

    // Enforce expected next character (None: accepts any valid character for the very first line)
    // Az elvárt következő karakter tartása (None: az legelső sornál még bármilyen érvényes karaktert elfogad)
    let mut expected_char: Option<u8> = None;

    loop {
        match port
            .read(&mut rx_buf)
            .with_timeout(Duration::from_millis(100))
            .await
        {
            Ok(Ok(len)) if len > 0 => {
                let byte = rx_buf[0];

                if line_len < line_buf.len() {
                    line_buf[line_len] = byte;
                    line_len += 1;
                } else {
                    line_len = 0;
                }

                // Check for CR + LF (\r\n) termination
                // CR + LF (\r\n) lezárás figyelése
                if line_len >= 2
                    && line_buf[line_len - 2] == b'\r'
                    && line_buf[line_len - 1] == b'\n'
                {
                    let payload = &line_buf[..line_len - 2];
                    let text = core::str::from_utf8(payload).unwrap_or("");

                    // 1. Check basic ASCII range (32..=126)
                    // 1. Alapvető ASCII tartomány ellenőrzése (32..=126)
                    let valid_ascii = payload.iter().all(|&b| (32..=126).contains(&b));

                    // 2. Get line internal consistency and its start character
                    // 2. A sor belső konzisztenciájának és a kezdőkarakterének lekérése
                    let current_char = if valid_ascii {
                        check_line_consistency(text)
                    } else {
                        None
                    };

                    // 3. Check sequence (33 -> 126 -> 33)
                    // 3. Sorrendiség ellenőrzése (33 -> 126 -> 33)
                    let mut is_valid_sequence = false;

                    if let Some(c) = current_char {
                        if let Some(expected) = expected_char {
                            if c == expected {
                                is_valid_sequence = true;
                            }
                        } else {
                            // For the very first line, accept if in the range 33..=126
                            // Legelső sor esetén elfogadjuk, ha 33..=126 tartományban van
                            if (33..=126).contains(&c) {
                                is_valid_sequence = true;
                            }
                        }

                        // Calculate next expected character (33 after 126)
                        // Következő várható karakter kiszámítása (126 után 33)
                        let next_expected = if c >= 126 { 33 } else { c + 1 };
                        expected_char = Some(next_expected);
                    } else {
                        // If the line was invalid, sequence continuity is also broken
                        // Ha a sor hibás volt, a sorrendiség folytonossága is megszakad
                        expected_char = None;
                    }

                    // 4. Log output
                    // 4. Kiírás a logba
                    if is_valid_sequence {
                        info!("Received line: {text}");
                    } else {
                        info!("Missing line or data line up to CR+LF contained an error.");
                    }

                    // 5. Flash status LED (Active Low)
                    // 5. Status LED villantása (Active Low)
                    status_led.set_low();
                    Timer::after(Duration::from_millis(100)).await;
                    status_led.set_high();
                    Timer::after(Duration::from_millis(100)).await;

                    // Flush buffer
                    // Puffer ürítése
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

/// Check row internal consistency and return the start character code if valid.
/// Ellenőrzi a sor belső konzisztenciáját és visszatér a kezdőkarakter kódjával, ha helyes.
fn check_line_consistency(line: &str) -> Option<u8> {
    let mut parts = line.split(',');

    // 1. Start character
    // 1. Kezdő karakter
    let first_part = parts.next()?.trim();
    if first_part.chars().count() != 1 {
        return None;
    }
    let target_char = first_part.chars().next()? as u8;
    let target_val = target_char as u32;

    // Read the following fields.
    // A következő mezők beolvasása
    let dec_part = parts.next()?;
    let hex_part = parts.next()?;
    let oct_part = parts.next()?;
    let bin_part = parts.next()?;

    // Trim prefixes and parse
    // Prefikszek levágása és parse-olás
    let parse_val = |part: &str, prefix: &str, radix: u32| -> Option<u32> {
        let val_str = part.trim().strip_prefix(prefix)?;
        u32::from_str_radix(val_str.trim(), radix).ok()
    };

    let dec_val = parse_val(dec_part, "dec:", 10)?;
    let hex_val = parse_val(hex_part, "hex:", 16)?;
    let oct_val = parse_val(oct_part, "oct:", 8)?;
    let bin_val = parse_val(bin_part, "bin:", 2)?;

    // Check internal numeral system consistency
    // Belső számrendszeri egyezőségek ellenőrzése
    if target_val == dec_val
        && target_val == hex_val
        && target_val == oct_val
        && target_val == bin_val
    {
        Some(target_char)
    } else {
        None
    }
}
