//! Native asynchronous serial port support for [`Interface`](crate::Interface).
//!
//! Uses the [`serial2-tokio`](https://crates.io/crates/serial2-tokio) crate.

extern crate std;

use crate::Error;
use embedded_io_adapters::tokio_1::FromTokio;
use embedded_io_async::ErrorType;
use serial2_tokio::{Parity, SerialPort, Settings};

/// Serial port type implementing [`Read`](embedded_io_async::Read)
/// and [`Write`](embedded_io_async::Write).
pub type Port = FromTokio<SerialPort>;

/// Port-specific error type to be used as `E` for the generic [`Error<E>`] type.
pub type PortError = <Port as ErrorType>::Error;

/// Opens a native serial port at the given path.
///
/// Returns a [`Port`] that can be passed to [`Interface::new`](crate::Interface::new).
pub fn open(path: &str) -> Result<Port, Error<std::io::Error>> {
    let port = SerialPort::open(path, |mut settings: Settings| {
        settings.set_raw();
        settings.set_baud_rate(2400)?;
        settings.set_parity(Parity::Even);

        Ok(settings)
    })?;

    port.discard_buffers()?;

    Ok(FromTokio::new(port))
}
