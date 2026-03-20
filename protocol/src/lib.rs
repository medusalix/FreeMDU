//! Communicate with Miele appliances via their proprietary diagnostic interface.
//!
//! # Overview
//!
//! The `freemdu` crate implements the proprietary Miele diagnostic protocol.
//! It offers an asynchronous, platform-agnostic API for communicating
//! with Miele appliances via the diagnostic interface.
//!
//! Depending on your needs, you can:
//!
//! - Use the high-level [`device`] module to query diagnostic properties and trigger actions.
//! - Instantiate device implementations (e.g. [`device::id629`]) to access model-specific methods.
//! - Work directly with the low-level diagnostic [`Interface`].
//!
//! # Getting started
//!
//! Most Miele appliances expose a diagnostic UART on their control board.
//! To communicate with it, you need a UART interface configured as follows:
//!
//! - **Baud rate:** 2400
//! - **Parity:** Even
//! - **Data bits:** 8
//! - **Stop bits:** 1
//!
//! If you enable the `native-serial` feature, you can obtain a compatible
//! serial port instance using [`serial::open`]:
//!
//! ```no_run
//! # #[tokio::main]
//! # async fn main() -> freemdu::device::Result<(), freemdu::serial::PortError> {
//! let mut port = freemdu::serial::open("/dev/ttyACM0")?;
//! # Ok(())
//! # }
//! ```
//!
//! The UART connection can be provided by a USB–UART adapter.
//! In that case, the adapter's RX, TX and GND lines must be connected to
//! the corresponding pins on the appliance's control board.
//!
//! <div class="warning">
//! Because the control board is typically not galvanically isolated,
//! working on it may expose you to dangerous voltages.
//! Always take appropriate safety precautions!
//! </div>
//!
//! Alternatively, you can access the interface through a compatible
//! **optical communication adapter**.
//! Instructions for building a simple adapter are available on the
//! [FreeMDU project page](https://github.com/medusalix/FreeMDU).
//!
//! # Examples
//!
//! The following examples demonstrate the primary ways to communicate with devices:
//!
//! ## Querying device properties using the high-level [`device`] module
//!
//! The recommended way to connect to a device is via [`device::connect`],
//! which identifies the device and provides access to its properties and actions:
//!
//! ```no_run
//! # #[tokio::main]
//! # async fn main() -> freemdu::device::Result<(), freemdu::serial::PortError> {
//! # let mut port = freemdu::serial::open("/dev/ttyACM0")?;
//! let mut dev = freemdu::device::connect(&mut port).await?;
//!
//! for prop in dev.properties() {
//!    let val = dev.query_property(prop).await?;
//!
//!    println!("{prop:?}: {val:?}");
//! }
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Working with a specific device implementation
//!
//! For model-specific access, a corresponding device instance has to be instantiated directly.
//! This provides additional methods beyond the general [`device::Device`] trait:
//!
//! ```no_run
//! use freemdu::device::{Device, id629::WashingMachine};
//!
//! # #[tokio::main]
//! # async fn main() -> freemdu::device::Result<(), freemdu::serial::PortError> {
//! # let mut port = freemdu::serial::open("/dev/ttyACM0")?;
//! let mut machine = WashingMachine::connect(&mut port).await?;
//!
//! println!("Program type: {}", machine.query_program_type().await?);
//! println!("Program options: {}", machine.query_program_options().await?);
//!
//! machine.start_program().await?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Low-level diagnostic access using [`Interface`]
//!
//! Advanced users can interact with the raw diagnostic protocol through the
//! low-level [`Interface`]:
//!
//! ```no_run
//! # #[tokio::main]
//! # async fn main() -> freemdu::Result<(), freemdu::serial::PortError> {
//! # let mut port = freemdu::serial::open("/dev/ttyACM0")?;
//! let mut intf = freemdu::Interface::new(port);
//!
//! println!("Software ID: {}", intf.query_software_id().await?);
//!
//! intf.unlock_read_access(0x1234).await?;
//! intf.unlock_full_access(0x5678).await?;
//!
//! let mem: [u8; 16] = intf.read_memory(0x0000).await?;
//!
//! println!("Memory contents: {:x?}", mem);
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Protocol details
//!
//! The diagnostic interface is protected by two **16-bit keys**.
//! Each appliance model or electronics board typically has
//! its own unique keys required to unlock the diagnostic interface.
//! The supported devices are listed on the
//! [FreeMDU project page](https://github.com/medusalix/FreeMDU).
//!
//! Once fully unlocked, the appliance accepts all diagnostic commands.
//! However, the interface automatically locks again after 3 seconds
//! of inactivity, so tools must send commands periodically to maintain access.

#![no_std]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

pub mod device;

#[cfg(feature = "native-serial")]
#[cfg_attr(docsrs, doc(cfg(feature = "native-serial")))]
pub mod serial;

pub use embedded_io_async;

use core::{
    fmt::{Debug, Display, Formatter},
    num::Wrapping,
};
use embedded_io_async::{Read, ReadExactError, Write};
use log::trace;
use strum::FromRepr;

/// A specialized [`Result`] type for [`Interface`] operations.
///
/// Uses [`Error<E>`] as the error variant, which can include port-specific errors.
pub type Result<T, E> = core::result::Result<T, Error<E>>;

/// Error type for [`Interface`] operations.
///
/// The generic parameter `E` allows the error type to carry a port-specific error.
///
/// This enum is marked `#[non_exhaustive]` to allow for future variants.
#[non_exhaustive]
#[derive(PartialEq, Eq, Debug)]
pub enum Error<E> {
    /// The provided argument is invalid.
    InvalidArgument,
    /// Data received by or from the device has an incorrect checksum.
    IncorrectChecksum,
    /// The device received an invalid command.
    InvalidCommand,
    /// The device returned an invalid response.
    InvalidResponse,
    /// The port encountered an unexpected end-of-file.
    UnexpectedEof,
    /// A port-specific input/output error.
    Io(E),
}

impl<E: core::error::Error> Display for Error<E> {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::IncorrectChecksum => write!(f, "incorrect checksum"),
            Self::InvalidCommand => write!(f, "invalid command"),
            Self::InvalidResponse => write!(f, "invalid response"),
            Self::UnexpectedEof => write!(f, "unexpected end-of-file"),
            Self::Io(err) => write!(f, "input/output error: {err}"),
        }
    }
}

impl<E: core::error::Error> core::error::Error for Error<E> {}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Self::Io(err)
    }
}

impl<E> From<ReadExactError<E>> for Error<E> {
    fn from(err: ReadExactError<E>) -> Self {
        match err {
            ReadExactError::UnexpectedEof => Self::UnexpectedEof,
            ReadExactError::Other(err) => Self::Io(err),
        }
    }
}

/// Baud rate used by the diagnostic interface.
#[derive(FromRepr, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum BaudRate {
    /// 2400 baud.
    Baud2400,
    /// 9600 baud.
    Baud9600,
    /// 19200 baud.
    Baud19200,
    /// 38400 baud.
    Baud38400,
    /// 57600 baud.
    Baud57600,
    /// 76800 baud.
    Baud76800,
    /// 115200 baud.
    Baud115200,
}

impl BaudRate {
    /// Returns the numeric baud rate value.
    #[must_use]
    pub const fn as_baud(self) -> u32 {
        match self {
            Self::Baud2400 => 2400,
            Self::Baud9600 => 9600,
            Self::Baud19200 => 19200,
            Self::Baud38400 => 38400,
            Self::Baud57600 => 57600,
            Self::Baud76800 => 76800,
            Self::Baud115200 => 115_200,
        }
    }
}

/// Command code used by the diagnostic interface.
#[derive(Debug)]
#[repr(u8)]
enum Command {
    Lock = 0x10,
    QuerySoftwareId = 0x11,
    UnlockReadAccess = 0x20,
    UnlockSmartHomeAccess = 0x21, // Available on newer devices
    ReadMemory = 0x30,
    ReadEeprom = 0x31,
    UnlockFullAccess = 0x32,
    ExtendAddress = 0x37,    // Available on newer devices
    QueryMaxBaudRate = 0x38, // Available on newer devices
    WriteMemory = 0x40,
    WriteEeprom = 0x41,
    JumpToSubroutine = 0x42,
    Halt = 0x45,
    SetBaudRate2400 = 0x46,
    SetBaudRate9600 = 0x47,
    SetChunkSize = 0x4a,     // Available on newer devices
    SetBaudRate = 0x4b,      // Available on newer devices
    Reset = 0x4e,            // Available on newer devices
    RequestSmartHome = 0x85, // Available on newer devices
}

/// Request message sent to the diagnostic interface.
///
/// A checksum must be appended to the serialized message using [`compute_checksum`].
#[derive(Debug)]
struct Request {
    cmd: Command,
    param: u16,
    len: u8,
}

impl Request {
    pub fn new(cmd: Command, param: u16, len: u8) -> Self {
        let req = Self { cmd, param, len };

        trace!("New request: {req:x?}");

        req
    }
}

impl From<Request> for Payload<4> {
    fn from(req: Request) -> Self {
        let mut buf = [0x00; 4];

        buf[0] = req.cmd as u8;
        buf[1..3].copy_from_slice(&req.param.to_le_bytes());
        buf[3] = req.len;

        Self(buf)
    }
}

/// Diagnostic interface response code.
///
/// Used in communication with the device, both when
/// sending requests and when interpreting responses.
#[derive(FromRepr, Debug)]
#[repr(u8)]
enum ResponseCode {
    Success,
    IncorrectChecksum,
    InvalidCommand,
}

/// Diagnostic interface payload.
///
/// Wraps a fixed-size byte array used for communication with the device.
/// When sent or received through an [`Interface`], the payload is automatically
/// split or reassembled into chunks.
///
/// Several [`From`] implementations are provided to convert between `Payload`
/// and common primitive types.
#[derive(Debug)]
pub struct Payload<const N: usize>([u8; N]);

impl<const N: usize> From<[u8; N]> for Payload<N> {
    fn from(data: [u8; N]) -> Self {
        Self(data)
    }
}

impl From<u8> for Payload<1> {
    fn from(val: u8) -> Self {
        Self(val.to_le_bytes())
    }
}

impl From<u16> for Payload<2> {
    fn from(val: u16) -> Self {
        Self(val.to_le_bytes())
    }
}

impl From<u32> for Payload<4> {
    fn from(val: u32) -> Self {
        Self(val.to_le_bytes())
    }
}

impl From<i8> for Payload<1> {
    fn from(val: i8) -> Self {
        Self(val.to_le_bytes())
    }
}

impl From<i16> for Payload<2> {
    fn from(val: i16) -> Self {
        Self(val.to_le_bytes())
    }
}

impl From<i32> for Payload<4> {
    fn from(val: i32) -> Self {
        Self(val.to_le_bytes())
    }
}

impl<const N: usize> From<Payload<N>> for [u8; N] {
    fn from(payload: Payload<N>) -> Self {
        payload.0
    }
}

impl From<Payload<1>> for u8 {
    fn from(payload: Payload<1>) -> Self {
        Self::from_le_bytes(payload.0)
    }
}

impl From<Payload<2>> for u16 {
    fn from(payload: Payload<2>) -> Self {
        Self::from_le_bytes(payload.0)
    }
}

impl From<Payload<4>> for u32 {
    fn from(payload: Payload<4>) -> Self {
        Self::from_le_bytes(payload.0)
    }
}

impl From<Payload<1>> for i8 {
    fn from(payload: Payload<1>) -> Self {
        Self::from_le_bytes(payload.0)
    }
}

impl From<Payload<2>> for i16 {
    fn from(payload: Payload<2>) -> Self {
        Self::from_le_bytes(payload.0)
    }
}

impl From<Payload<4>> for i32 {
    fn from(payload: Payload<4>) -> Self {
        Self::from_le_bytes(payload.0)
    }
}

fn compute_checksum(data: &[u8]) -> u8 {
    data.iter().map(|&x| Wrapping(x)).sum::<Wrapping<_>>().0
}

/// Asynchronous diagnostic protocol interface.
///
/// Requires a port that implements [`Read`] and [`Write`] for communication.
///
/// Most users should access devices through the [`device`] module:
///
/// - Use [`device::connect`] to obtain a [`device::Device`] trait object with
///   high-level methods for querying properties and triggering actions.
/// - Alternatively, use one of the [`device`] submodules directly if you only need
///   support for a specific device.
///
/// [`Interface`] is only intended for advanced use cases where direct,
/// low-level access to the diagnostic protocol is required.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::Result<(), freemdu::serial::PortError> {
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut intf = freemdu::Interface::new(port);
///
/// println!("Software ID: {}", intf.query_software_id().await?);
///
/// intf.unlock_read_access(0x1234).await?;
/// intf.unlock_full_access(0x5678).await?;
///
/// let mem: [u8; 16] = intf.read_memory(0x0000).await?;
///
/// println!("Memory: {:x?}", mem);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Interface<P> {
    port: P,
    chunk_size: u8,
}

impl<P: Read + Write> Interface<P> {
    /// Constructs a new diagnostic interface.
    pub fn new(port: P) -> Self {
        Self {
            port,
            chunk_size: 4, // Default size, adjustable on newer devices
        }
    }

    /// Locks the diagnostic interface.
    ///
    /// This command resets the device's diagnostic access level.
    /// After locking, any further diagnostic operations require
    /// repeating the full unlock sequence.
    pub async fn lock(&mut self) -> Result<(), P::Error> {
        self.send(Request::new(Command::Lock, 0x0000, 0x00).into())
            .await
    }

    /// Queries the software ID of the device.
    ///
    /// This number identifies the software/firmware running on the device.
    /// Note that different electronics boards can share the same software ID.
    ///
    /// This must be called first as part of the unlock sequence.
    /// See [`Interface::unlock_read_access`] for the next step.
    pub async fn query_software_id(&mut self) -> Result<u16, P::Error> {
        self.send(Request::new(Command::QuerySoftwareId, 0x0000, 0x02).into())
            .await?;

        Ok(self.receive().await?.into())
    }

    /// Unlocks read-only diagnostic access.
    ///
    /// Before calling this function, the software ID must be
    /// queried using [`Interface::query_software_id`].
    /// Diagnostic access requires a key, which is typically unique for each software ID.
    ///
    /// Successfully unlocking read access enables the following functions:
    ///
    /// - [`Interface::read_memory`]
    /// - [`Interface::read_eeprom`]
    /// - [`Interface::query_max_baud_rate`]
    /// - [`Interface::send_smart_home_request`]
    pub async fn unlock_read_access(&mut self, key: u16) -> Result<(), P::Error> {
        self.send(Request::new(Command::UnlockReadAccess, key, 0x00).into())
            .await
    }

    /// Unlocks access to the smart home functionality.
    ///
    /// Smart home functionality is only supported on newer devices.
    ///
    /// Before calling this function, the software ID must be
    /// queried using [`Interface::query_software_id`].
    /// Unlike diagnostic access, unlocking smart home features does not
    /// require a device-specific key.
    ///
    /// Successfully unlocking smart home access enables the following functions:
    ///
    /// - [`Interface::query_max_baud_rate`]
    /// - [`Interface::set_baud_rate`]
    /// - [`Interface::set_chunk_size`]
    /// - [`Interface::send_smart_home_request`]
    pub async fn unlock_smart_home_access(&mut self) -> Result<(), P::Error> {
        self.send(Request::new(Command::UnlockSmartHomeAccess, 0x0000, 0x00).into())
            .await
    }

    /// Reads data from the device's memory.
    ///
    /// Newer devices support reading up to 65535 bytes from a 32-bit memory address,
    /// while older devices are limited to 255 bytes and 16-bit addresses.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if the payload length exceeds 65535 bytes.
    pub async fn read_memory<L: From<Payload<N>>, const N: usize>(
        &mut self,
        addr: u32,
    ) -> Result<L, P::Error> {
        let len: u16 = N.try_into().map_err(|_| Error::InvalidArgument)?;

        // Send upper bytes of address or length
        if addr > 0xffff || len > 0xff {
            self.send(
                Request::new(
                    Command::ExtendAddress,
                    (addr >> 16) as u16,
                    (len >> 8) as u8,
                )
                .into(),
            )
            .await?;
        }

        self.send(
            Request::new(
                Command::ReadMemory,
                (addr & 0xffff) as u16,
                (len & 0xff) as u8,
            )
            .into(),
        )
        .await?;

        Ok(self.receive().await?.into())
    }

    /// Reads data from the device's EEPROM.
    ///
    /// For older devices, the address must be specified in words, not bytes.
    /// As an example, to read a byte at address `0x64`, provide the word address `0x32`.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if the payload length exceeds 255 bytes.
    pub async fn read_eeprom<L: From<Payload<N>>, const N: usize>(
        &mut self,
        addr: u16,
    ) -> Result<L, P::Error> {
        let len = N.try_into().map_err(|_| Error::InvalidArgument)?;

        self.send(Request::new(Command::ReadEeprom, addr, len).into())
            .await?;

        Ok(self.receive().await?.into())
    }

    /// Queries the device's maximum supported baud rate.
    ///
    /// The maximum baud rate can only be queried on newer devices.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidResponse`] if the device responds with an invalid baud rate
    pub async fn query_max_baud_rate(&mut self) -> Result<BaudRate, P::Error> {
        self.send(Request::new(Command::QueryMaxBaudRate, 0x0000, 0x02).into())
            .await?;

        let resp: [u8; 2] = self.receive().await?.into();

        BaudRate::from_repr(resp[1]).ok_or(Error::InvalidResponse)
    }

    /// Unlocks full diagnostic access.
    ///
    /// Before calling this function, read-only access has to be
    /// unlocked using [`Interface::unlock_read_access`].
    /// Diagnostic access requires a key, which is typically unique for each software ID.
    ///
    /// Successfully unlocking full access enables the following functions:
    ///
    /// - [`Interface::write_memory`]
    /// - [`Interface::write_eeprom`]
    /// - [`Interface::jump_to_subroutine`]
    /// - [`Interface::halt`]
    /// - [`Interface::set_baud_rate`]
    /// - [`Interface::set_chunk_size`]
    /// - [`Interface::reset`]
    pub async fn unlock_full_access(&mut self, key: u16) -> Result<(), P::Error> {
        self.send(Request::new(Command::UnlockFullAccess, key, 0x00).into())
            .await
    }

    /// Writes data to the device's memory.
    ///
    /// Newer devices support writing up to 65535 bytes to a 32-bit memory address,
    /// while older devices are limited to 255 bytes and 16-bit addresses.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if the payload length exceeds 65535 bytes.
    pub async fn write_memory<L: Into<Payload<N>>, const N: usize>(
        &mut self,
        addr: u32,
        payload: L,
    ) -> Result<(), P::Error> {
        let len: u16 = N.try_into().map_err(|_| Error::InvalidArgument)?;

        // Send upper bytes of address or length
        if addr > 0xffff || len > 0xff {
            self.send(
                Request::new(
                    Command::ExtendAddress,
                    (addr >> 16) as u16,
                    (len >> 8) as u8,
                )
                .into(),
            )
            .await?;
        }

        self.send(
            Request::new(
                Command::WriteMemory,
                (addr & 0xffff) as u16,
                (len & 0xff) as u8,
            )
            .into(),
        )
        .await?;
        self.send(payload.into()).await
    }

    /// Writes data to the device's EEPROM.
    ///
    /// For older devices, the address must be specified in words, not bytes.
    /// As an example, to write a byte at address `0x64`, provide the word address `0x32`.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if the payload length exceeds 255 bytes.
    pub async fn write_eeprom<L: Into<Payload<N>>, const N: usize>(
        &mut self,
        addr: u16,
        payload: L,
    ) -> Result<(), P::Error> {
        let len = N.try_into().map_err(|_| Error::InvalidArgument)?;

        self.send(Request::new(Command::WriteEeprom, addr, len).into())
            .await?;
        self.send(payload.into()).await
    }

    /// Jumps to a specified subroutine and waits for it to return.
    ///
    /// Newer devices support jumping to a 32-bit memory address,
    /// while older devices are limited to 16-bit addresses.
    ///
    /// This resets the device's diagnostic access level.
    /// The interface must be unlocked again after this operation
    /// to perform further diagnostic commands.
    pub async fn jump_to_subroutine(&mut self, addr: u32) -> Result<(), P::Error> {
        // Send upper bytes of address
        if addr > 0xffff {
            self.send(Request::new(Command::ExtendAddress, (addr >> 16) as u16, 0x00).into())
                .await?;
        }

        // Response is sent once subroutine returns
        self.send(Request::new(Command::JumpToSubroutine, (addr & 0xffff) as u16, 0x00).into())
            .await?;
        self.read(&mut [0x00]).await
    }

    /// Halts the device's normal operation.
    ///
    /// Causes the device to enter an infinite loop.
    pub async fn halt(&mut self) -> Result<(), P::Error> {
        self.send(Request::new(Command::Halt, 0x0000, 0x00).into())
            .await
    }

    /// Sets the device's baud rate.
    ///
    /// Baud rates above 9600 baud are only supported on newer devices.
    /// On these devices, the maximum supported baud rate
    /// can be queried via [`Interface::query_max_baud_rate`].
    /// If a higher-than-supported baud rate is requested on a newer device,
    /// it will automatically fall back to the highest supported baud rate.
    ///
    /// This resets the device's diagnostic access level.
    /// The interface must be unlocked again after this operation
    /// to perform further diagnostic commands.
    ///
    /// Note that this does not change the baud rate of the current port instance.
    /// A new [`Interface`] must be created with a port configured for the selected baud rate.
    pub async fn set_baud_rate(&mut self, rate: BaudRate) -> Result<(), P::Error> {
        match rate {
            BaudRate::Baud2400 => {
                self.send(Request::new(Command::SetBaudRate2400, 0x0000, 0x00).into())
                    .await
            }
            BaudRate::Baud9600 => {
                self.send(Request::new(Command::SetBaudRate9600, 0x0000, 0x00).into())
                    .await
            }
            _ => {
                self.send(Request::new(Command::SetBaudRate, rate as u16, 0x01).into())
                    .await?;

                // Device responds with actual baud rate
                let _: u8 = self.receive().await?.into();

                Ok(())
            }
        }
    }

    /// Configures the diagnostic frame chunk size.
    ///
    /// The chunk size can only be adjusted on newer devices.
    ///
    /// If the requested size is outside the supported range,
    /// it is clamped by the device to the nearest supported boundary.
    /// The supported range is device-specific,
    /// but is typically between 4 and 128 bytes.
    pub async fn set_chunk_size(&mut self, size: u8) -> Result<(), P::Error> {
        self.send(Request::new(Command::SetChunkSize, u16::from(size), 0x01).into())
            .await?;

        // Device responds with actual chunk size
        self.chunk_size = self.receive().await?.into();

        Ok(())
    }

    /// Resets the device's microcontroller.
    ///
    /// A reset can only be performed on newer devices.
    pub async fn reset(&mut self) -> Result<(), P::Error> {
        self.send(Request::new(Command::Reset, 0x0000, 0x00).into())
            .await
    }

    /// Sends a smart home request to the device and returns the response.
    ///
    /// Smart home functionality is only supported on newer devices.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if the payload length exceeds 255 bytes.
    pub async fn send_smart_home_request<const M: usize, const N: usize>(
        &mut self,
        cmd: u16,
        payload: Payload<N>,
    ) -> Result<Payload<M>, P::Error> {
        let len = N.try_into().map_err(|_| Error::InvalidArgument)?;

        self.send(Request::new(Command::RequestSmartHome, cmd, len).into())
            .await?;
        self.send(payload).await?;
        self.receive().await
    }

    /// Sends a payload to the port.
    ///
    /// The payload is split into chunks with an appended checksum.
    /// Chunks are sent sequentially, verifying the response code for every transmission.
    async fn send<const N: usize>(&mut self, payload: Payload<N>) -> Result<(), P::Error> {
        for chunk in payload.0.chunks(self.chunk_size as usize) {
            let checksum = compute_checksum(chunk);
            let mut resp = [0xff];

            self.write(chunk).await?;
            self.write(&[checksum]).await?;
            self.read(&mut resp).await?;

            match ResponseCode::from_repr(resp[0]) {
                Some(ResponseCode::Success) => Ok(()),
                Some(ResponseCode::IncorrectChecksum) => Err(Error::IncorrectChecksum),
                Some(ResponseCode::InvalidCommand) => Err(Error::InvalidCommand),
                None => Err(Error::InvalidResponse),
            }?;
        }

        Ok(())
    }

    /// Receives a payload from the port.
    ///
    /// Chunks of the payload are read and their checksums verified.
    /// A response code is sent for every received chunk.
    async fn receive<const N: usize>(&mut self) -> Result<Payload<N>, P::Error> {
        let mut payload = Payload([0x00; N]);

        for chunk in payload.0.chunks_mut(self.chunk_size as usize) {
            let mut checksum = [0x00];

            self.read(chunk).await?;
            self.read(&mut checksum).await?;

            if checksum[0] != compute_checksum(chunk) {
                return Err(Error::IncorrectChecksum);
            }

            // Acknowledge reception of chunk
            // Sending other response codes here aborts the transfer
            self.write(&[ResponseCode::Success as u8]).await?;
        }

        Ok(payload)
    }

    /// Reads data from the port into the provided buffer.
    async fn read(&mut self, buf: &mut [u8]) -> Result<(), P::Error> {
        self.port.read_exact(buf).await?;
        trace!("Read from port: {buf:02x?}");

        Ok(())
    }

    /// Writes the provided buffer to the port.
    async fn write(&mut self, buf: &[u8]) -> Result<(), P::Error> {
        trace!("Write to port: {buf:02x?}");
        self.port.write_all(buf).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{boxed::Box, collections::vec_deque::VecDeque};
    use core::convert::Infallible;
    use log::LevelFilter;

    pub fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::max())
            .is_test(true)
            .try_init();
    }

    #[tokio::test]
    async fn lock() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.lock().await?;

        assert_eq!(
            deque,
            [0x10, 0x00, 0x00, 0x00, 0x10],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn query_software_id() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x75, 0x02, 0x77]);
        let mut intf = Interface::new(&mut deque);
        let id = intf.query_software_id().await?;

        assert_eq!(id, 629, "software ID should be correct");
        assert_eq!(
            deque,
            [0x11, 0x00, 0x00, 0x02, 0x13, 0x00],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn unlock_read_access() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.unlock_read_access(0xabcd).await?;

        assert_eq!(
            deque,
            [0x20, 0xcd, 0xab, 0x00, 0x98],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn unlock_smart_home_access() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.unlock_smart_home_access().await?;

        assert_eq!(
            deque,
            [0x21, 0x00, 0x00, 0x00, 0x21],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn read_memory() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([
            0x00, 0x11, 0x22, 0x33, 0x44, 0xaa, 0xab, 0xcd, 0xef, 0x99, 0x00, 0xde, 0xad, 0x8b,
        ]);
        let mut intf = Interface::new(&mut deque);
        let data: [u8; 10] = intf.read_memory(0xabcd).await?;

        assert_eq!(
            deque,
            [0x30, 0xcd, 0xab, 0x0a, 0xb2, 0x00, 0x00, 0x00],
            "deque contents should be correct"
        );

        assert_eq!(
            data,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
            "memory contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn read_memory_extended() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([
            0x00, 0x00, 0x11, 0x22, 0x33, 0x44, 0xaa, 0xab, 0xcd, 0xef, 0x99, 0x00, 0xde, 0xad,
            0x8b,
        ]);
        let mut intf = Interface::new(&mut deque);
        let data: [u8; 10] = intf.read_memory(0x1234_abcd).await?;

        assert_eq!(
            deque,
            [
                0x37, 0x34, 0x12, 0x00, 0x7d, 0x30, 0xcd, 0xab, 0x0a, 0xb2, 0x00, 0x00, 0x00
            ],
            "deque contents should be correct"
        );

        assert_eq!(
            data,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
            "memory contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn read_eeprom() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([
            0x00, 0x11, 0x22, 0x33, 0x44, 0xaa, 0xab, 0xcd, 0xef, 0x99, 0x00, 0xde, 0xad, 0x8b,
        ]);
        let mut intf = Interface::new(&mut deque);
        let data: [u8; 10] = intf.read_eeprom(0xabcd).await?;

        assert_eq!(
            deque,
            [0x31, 0xcd, 0xab, 0x0a, 0xb3, 0x00, 0x00, 0x00],
            "deque contents should be correct"
        );

        assert_eq!(
            data,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
            "EEPROM contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn query_max_baud_rate() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x80, 0x03, 0x83]);
        let mut intf = Interface::new(&mut deque);
        let rate = intf.query_max_baud_rate().await?;

        assert_eq!(
            deque,
            [0x38, 0x00, 0x00, 0x02, 0x3a, 0x00],
            "deque contents should be correct"
        );

        assert_eq!(rate, BaudRate::Baud38400, "baud rate should be correct");

        Ok(())
    }

    #[tokio::test]
    async fn unlock_full_access() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.unlock_full_access(0xabcd).await?;

        assert_eq!(
            deque,
            [0x32, 0xcd, 0xab, 0x00, 0xaa],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn write_memory() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x00, 0x00, 0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.write_memory(
            0xabcd,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
        )
        .await?;

        assert_eq!(
            deque,
            [
                0x40, 0xcd, 0xab, 0x0a, 0xc2, 0x11, 0x22, 0x33, 0x44, 0xaa, 0xab, 0xcd, 0xef, 0x99,
                0x00, 0xde, 0xad, 0x8b
            ],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn write_memory_extended() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x00, 0x00, 0x00, 0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.write_memory(
            0x1234_abcd,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
        )
        .await?;

        assert_eq!(
            deque,
            [
                0x37, 0x34, 0x12, 0x00, 0x7d, 0x40, 0xcd, 0xab, 0x0a, 0xc2, 0x11, 0x22, 0x33, 0x44,
                0xaa, 0xab, 0xcd, 0xef, 0x99, 0x00, 0xde, 0xad, 0x8b
            ],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn write_eeprom() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x00, 0x00, 0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.write_eeprom(
            0xabcd,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
        )
        .await?;

        assert_eq!(
            deque,
            [
                0x41, 0xcd, 0xab, 0x0a, 0xc3, 0x11, 0x22, 0x33, 0x44, 0xaa, 0xab, 0xcd, 0xef, 0x99,
                0x00, 0xde, 0xad, 0x8b
            ],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn jump_to_subroutine() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.jump_to_subroutine(0xabcd).await?;

        assert_eq!(
            deque,
            [0x42, 0xcd, 0xab, 0x00, 0xba],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn jump_to_subroutine_extended() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x00, 0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.jump_to_subroutine(0x1234_abcd).await?;

        assert_eq!(
            deque,
            [0x37, 0x34, 0x12, 0x00, 0x7d, 0x42, 0xcd, 0xab, 0x00, 0xba],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn halt() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.halt().await?;

        assert_eq!(
            deque,
            [0x45, 0x00, 0x00, 0x00, 0x45],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_baud_rate_2400() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.set_baud_rate(BaudRate::Baud2400).await?;

        assert_eq!(
            deque,
            [0x46, 0x00, 0x00, 0x00, 0x46],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_baud_rate_9600() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.set_baud_rate(BaudRate::Baud9600).await?;

        assert_eq!(
            deque,
            [0x47, 0x00, 0x00, 0x00, 0x47],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_chunk_size() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([
            0x00, 0x80, 0x80, 0x00, 0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad,
            0x35,
        ]);
        let mut intf = Interface::new(&mut deque);

        intf.set_chunk_size(128).await?;

        let data: [u8; 10] = intf.read_memory(0xabcd).await?;

        assert_eq!(
            deque,
            [
                0x4a, 0x80, 0x0, 0x1, 0xcb, 0x00, 0x30, 0xcd, 0xab, 0x0a, 0xb2, 0x00
            ],
            "deque contents should be correct"
        );

        assert_eq!(
            data,
            [0x11, 0x22, 0x33, 0x44, 0xab, 0xcd, 0xef, 0x99, 0xde, 0xad],
            "memory contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_baud_rate() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x02, 0x02]);
        let mut intf = Interface::new(&mut deque);

        intf.set_baud_rate(BaudRate::Baud19200).await?;

        assert_eq!(
            deque,
            [0x4b, 0x02, 0x00, 0x01, 0x4e, 0x00],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn reset() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);

        intf.reset().await?;

        assert_eq!(
            deque,
            [0x4e, 0x00, 0x00, 0x00, 0x4e],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn send_smart_home_request() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x00, 0x00, 0x00, 0x00]);
        let mut intf = Interface::new(&mut deque);
        let payload: Payload<2> = intf
            .send_smart_home_request(0x0001, [0x00, 0x03].into())
            .await?;

        assert_eq!(
            deque,
            [0x85, 0x01, 0x00, 0x02, 0x88, 0x00, 0x03, 0x03, 0x00],
            "deque contents should be correct"
        );

        assert_eq!(payload.0, [0x00, 0x00], "response should be correct");

        Ok(())
    }

    #[tokio::test]
    async fn error_invalid_argument() -> Result<(), Infallible> {
        static DATA: [u8; 65536] = [0x00; _];

        init_logger();

        let mut deque = VecDeque::from([]);
        let mut intf = Interface::new(&mut deque);
        let res: Result<[u8; 65536], _> = Box::pin(intf.read_memory(0xabcd)).await;

        assert_eq!(
            res.unwrap_err(),
            Error::InvalidArgument,
            "result should be invalid argument error"
        );

        let res = Box::pin(intf.write_memory(0xabcd, DATA)).await;

        assert_eq!(
            res.unwrap_err(),
            Error::InvalidArgument,
            "result should be invalid argument error"
        );

        Ok(())
    }

    #[tokio::test]
    async fn error_incorrect_checksum() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x01, 0x00, 0x11, 0xff]);
        let mut intf = Interface::new(&mut deque);
        let res = intf.lock().await;

        assert_eq!(
            res.unwrap_err(),
            Error::IncorrectChecksum,
            "result should be incorrect checksum error"
        );

        let res: Result<u8, _> = intf.read_memory(0xabcd).await;

        assert_eq!(
            res.unwrap_err(),
            Error::IncorrectChecksum,
            "result should be incorrect checksum error"
        );

        Ok(())
    }

    #[tokio::test]
    async fn error_invalid_command() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x02]);
        let mut intf = Interface::new(&mut deque);
        let res = intf.lock().await;

        assert_eq!(
            res.unwrap_err(),
            Error::InvalidCommand,
            "result should be invalid command error"
        );

        Ok(())
    }

    #[tokio::test]
    async fn error_unknown_response_code() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0xff]);
        let mut intf = Interface::new(&mut deque);
        let res = intf.halt().await;

        assert_eq!(
            res.unwrap_err(),
            Error::InvalidResponse,
            "result should be invalid response error"
        );

        Ok(())
    }

    #[tokio::test]
    async fn error_unexpected_eof() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00]);
        let mut intf = Interface::new(&mut deque);
        let res: Result<[u8; 5], _> = intf.read_memory(0xabcd).await;

        assert_eq!(
            res.unwrap_err(),
            Error::UnexpectedEof,
            "result should be unexpected end-of-file error"
        );

        Ok(())
    }
}
