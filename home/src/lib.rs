#![no_std]

use embedded_io_async::{ErrorType, Read, ReadExactError, Write};
#[cfg(feature = "ws2812led")]
use esp_hal::peripherals::RMT;
use esp_hal::{
    Async,
    gpio::{AnyPin, Input, InputConfig, Level, Output, OutputConfig},
    uart::{Config, ConfigError, Instance, IoError, Parity, Uart},
};

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
        // Retry on error
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

        // Discard data that is read back by the optical receiver
        for _ in 0..len {
            self.read(&mut [0x00]).await?;
        }

        Ok(len)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(self.0.flush_async().await?)
    }
}

pub mod status_led {
    use esp_hal::gpio::Output;
    #[cfg(feature = "ws2812led")]
    use {
        esp_hal::{Blocking, peripherals::RMT, rmt::Rmt, time::Rate},
        esp_hal_smartled::{Ws2812SmartLeds, buffer_size},
        smart_leds::{
            RGB8, SmartLedsWrite,
            colors::{BLACK, GREEN, RED},
        },
    };

    // Define the internal driver based on the feature.
    enum LedDriver<'a> {
        #[cfg(feature = "ws2812led")]
        Smart(Ws2812SmartLeds<'a, { buffer_size::<RGB8>(1) }, Blocking>),
        Plain(Output<'a>),
    }

    pub struct StatusLed<'a> {
        driver: LedDriver<'a>,
    }

    impl<'a> StatusLed<'a> {
        #[cfg(feature = "ws2812led")]
        pub fn new_smart(rmt: RMT<'a>, output: Output<'a>) -> Self {
            let rmt = Rmt::new(rmt, Rate::from_mhz(80)).unwrap();
            let led = Ws2812SmartLeds::new(rmt.channel0, output).unwrap();
            Self {
                driver: LedDriver::Smart(led),
            }
        }

        pub fn new_plain(output: Output<'a>) -> Self {
            Self {
                driver: LedDriver::Plain(output),
            }
        }

        pub fn on(&mut self) {
            match &mut self.driver {
                #[cfg(feature = "ws2812led")]
                LedDriver::Smart(led) => {
                    led.write([RED]).ok();
                }
                LedDriver::Plain(pin) => pin.set_low(),
            }
        }

        pub fn off(&mut self) {
            match &mut self.driver {
                #[cfg(feature = "ws2812led")]
                LedDriver::Smart(led) => {
                    led.write([BLACK]).ok();
                }
                LedDriver::Plain(pin) => pin.set_high(),
            }
        }

        pub fn set_connected(&mut self, connected: bool) {
            match &mut self.driver {
                #[cfg(feature = "ws2812led")]
                LedDriver::Smart(led) => {
                    led.write([if connected { GREEN } else { BLACK }]).ok();
                }
                LedDriver::Plain(pin) => pin.set_level((!connected).into()),
            }
        }
    }
}

#[must_use]
pub fn new_status_led<'a>(#[cfg(feature = "ws2812led")] rmt: RMT<'a>) -> status_led::StatusLed<'a> {
    const PIN: u8 = num_from_env!("PIN_LED_STATUS", u8);
    let led = unsafe { AnyPin::steal(PIN) };
    let output = Output::new(led, Level::High, OutputConfig::default());

    #[cfg(feature = "ws2812led")]
    {
        status_led::StatusLed::new_smart(rmt, output)
    }
    #[cfg(not(feature = "ws2812led"))]
    {
        status_led::StatusLed::new_plain(output)
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
