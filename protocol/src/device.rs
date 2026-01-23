//! High-level device abstractions.
//!
//! Provides a [`Device`] trait implemented by all supported appliances,
//! such as washing machines, tumble dryers, and coffee machines.
//! The trait abstracts over differences between devices, providing
//! a unified interface for querying properties and triggering actions.
//!
//! Use the [`connect`] function to automatically select the correct device
//! implementation based on the devices's software ID.

pub mod id132;
pub mod id2088;
pub mod id218;
pub mod id2895;
pub mod id324;
pub mod id360;
pub mod id419;
pub mod id469;
pub mod id605;
pub mod id629;
pub(super) mod utils;

use crate::{Error as ProtocolError, Interface, Read, Write};
use alloc::{boxed::Box, string::String};
use core::{
    fmt::{Display, Formatter},
    num::TryFromIntError,
    time::Duration,
};

/// A specialized [`Result`] type for [`Device`] operations.
///
/// Uses [`Error<E>`] as the error variant, which can include port-specific errors.
pub type Result<T, E> = core::result::Result<T, Error<E>>;

/// Error type for [`Device`] operations.
///
/// The generic parameter `E` allows the error type to carry a port-specific error.
///
/// This enum is marked `#[non_exhaustive]` to allow for future variants.
#[non_exhaustive]
#[derive(PartialEq, Eq, Debug)]
pub enum Error<E> {
    /// The software ID is unknown or does not match the expected value.
    UnknownSoftwareId(u16),
    /// The provided argument is invalid.
    InvalidArgument,
    /// The device was in an invalid state for the requested operation.
    InvalidState,
    /// The device memory contains an unexpected value.
    UnexpectedMemoryValue,
    /// An unknown device property was queried.
    UnknownProperty,
    /// An unrecognized device action was requested.
    UnknownAction,
    /// Generic diagnostic protocol error.
    Protocol(ProtocolError<E>),
}

impl<E: core::error::Error> Display for Error<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownSoftwareId(id) => write!(f, "unknown software ID: {id}"),
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::InvalidState => write!(f, "invalid state"),
            Self::UnexpectedMemoryValue => write!(f, "unexpected memory value"),
            Self::UnknownProperty => write!(f, "unknown property"),
            Self::UnknownAction => write!(f, "unknown action"),
            Self::Protocol(err) => write!(f, "protocol error: {err}"),
        }
    }
}

impl<E: core::error::Error> core::error::Error for Error<E> {}

impl<E> From<ProtocolError<E>> for Error<E> {
    fn from(err: ProtocolError<E>) -> Self {
        Self::Protocol(err)
    }
}

impl<E> From<TryFromIntError> for Error<E> {
    fn from(_err: TryFromIntError) -> Self {
        Self::UnexpectedMemoryValue
    }
}

impl<E> From<bitflags::parser::ParseError> for Error<E> {
    fn from(_err: bitflags::parser::ParseError) -> Self {
        Self::InvalidArgument
    }
}

impl<E> From<strum::ParseError> for Error<E> {
    fn from(_err: strum::ParseError) -> Self {
        Self::InvalidArgument
    }
}

/// Device kind.
///
/// Currently includes common appliance types.
///
/// This enum is marked `#[non_exhaustive]` to allow for future variants.
#[non_exhaustive]
#[derive(strum::Display, PartialEq, Eq, Copy, Clone, Debug)]
#[strum(serialize_all = "title_case")]
pub enum DeviceKind {
    /// Washing machine.
    WashingMachine,
    /// Tumble dryer.
    TumbleDryer,
    /// Washer-dryer combination.
    WasherDryer,
    /// Dishwasher.
    Dishwasher,
    /// Coffee machine.
    CoffeeMachine,
}

/// Device property kind.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum PropertyKind {
    /// General properties, e.g. model number.
    General,
    /// Fault properties, e.g. detergent overdose.
    Fault,
    /// Operation properties, e.g. program phase.
    Operation,
    /// Input/output properties, e.g. water level.
    Io,
}

/// A device property, e.g. total operating time.
///
/// Properties can be queried using [`Device::query_property`].
#[derive(PartialEq, Eq, Debug)]
pub struct Property {
    /// Property kind.
    pub kind: PropertyKind,
    /// Unique identifier.
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// Optional unit of the property's value.
    pub unit: Option<&'static str>,
}

/// Device action kind.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ActionKind {
    /// Operation action, e.g. setting the program options.
    Operation,
    /// Calibration action, e.g. calibrating the water level.
    Calibration,
}

/// Expected parameter types for a device action.
///
/// Each variant specifies which kind of [`Value`] must be supplied
/// when invoking [`Device::trigger_action`].
#[derive(PartialEq, Eq, Debug)]
pub enum ActionParameters {
    /// Action accepts a single value chosen from an enumeration.
    ///
    /// The slice contains all possible options.
    Enumeration(&'static [&'static str]),
    /// Action accepts a value representing a combination of flags.
    ///
    /// The slice contains all possible flag names.
    Flags(&'static [&'static str]),
}

/// A device action, e.g. starting the current washing program.
///
/// Triggered via [`Device::trigger_action`].
#[derive(PartialEq, Eq, Debug)]
pub struct Action {
    /// Action kind.
    pub kind: ActionKind,
    /// Unique identifier.
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// Expected parameters, if any.
    pub params: Option<ActionParameters>,
}

/// The value of a device property or action argument.
///
/// Returned by [`Device::query_property`] or passed to [`Device::trigger_action`].
/// The type depends on the queried property or triggered action.
#[derive(PartialEq, Eq, Debug)]
pub enum Value {
    /// Boolean value.
    Bool(bool),
    /// Number value.
    Number(u32),
    /// Sensor reading (current and target values).
    Sensor(u32, u32),
    /// String value of arbitrary length.
    String(String),
    /// Duration value.
    Duration(Duration),
    /// Date value.
    Date(Date),
    /// Fault value.
    Fault(Fault),
}

/// A simple date, consisting of year, month and day.
#[derive(PartialEq, Eq, Debug)]
pub struct Date {
    /// Year value.
    pub year: u16,
    /// Month value.
    pub month: u8,
    /// Day value.
    pub day: u8,
}

impl Date {
    /// Constructs a new date.
    #[must_use]
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }
}

/// Additional information about a fault.
#[derive(PartialEq, Eq, Debug)]
pub struct FaultInfo {
    /// Last time of occurrence.
    pub operating_hours: u32,
    /// Number of occurrences.
    pub count: u32,
}

/// The status of a device fault.
///
/// Some devices provide additional metadata for active or stored faults using [`FaultInfo`].
#[derive(PartialEq, Eq, Debug)]
pub enum Fault {
    /// Fault is not asserted.
    Ok,
    /// Fault is currently active.
    ///
    /// May include additional information if available.
    Active(Option<FaultInfo>),
    /// Fault is stored in non-volatile memory (EEPROM).
    ///
    /// May include additional information if available.
    Stored(Option<FaultInfo>),
}

impl From<bool> for Value {
    fn from(val: bool) -> Self {
        Self::Bool(val)
    }
}

impl From<u8> for Value {
    fn from(val: u8) -> Self {
        Self::Number(val.into())
    }
}

impl From<u16> for Value {
    fn from(val: u16) -> Self {
        Self::Number(val.into())
    }
}

impl From<u32> for Value {
    fn from(val: u32) -> Self {
        Self::Number(val)
    }
}

impl From<(u8, u8)> for Value {
    fn from(vals: (u8, u8)) -> Self {
        Self::Sensor(vals.0.into(), vals.1.into())
    }
}

impl From<(u16, u16)> for Value {
    fn from(vals: (u16, u16)) -> Self {
        Self::Sensor(vals.0.into(), vals.1.into())
    }
}

impl From<(u32, u32)> for Value {
    fn from(vals: (u32, u32)) -> Self {
        Self::Sensor(vals.0, vals.1)
    }
}

impl From<String> for Value {
    fn from(string: String) -> Self {
        Self::String(string)
    }
}

impl From<Duration> for Value {
    fn from(dur: Duration) -> Self {
        Self::Duration(dur)
    }
}

impl From<Date> for Value {
    fn from(date: Date) -> Self {
        Self::Date(date)
    }
}

impl From<Fault> for Value {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

/// Trait implemented by all supported devices.
///
/// Provides asynchronous access to device properties and actions
/// over a diagnostic port that implements [`Read`] and [`Write`].
///
/// This trait is sealed and cannot be implemented outside this crate.
///
/// # Errors
///
/// - [`Error::Protocol`] for any errors during diagnostic communication.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut dev = freemdu::device::connect(&mut port).await?;
///
/// for prop in dev.properties() {
///     let val = dev.query_property(prop).await?;
///
///     println!("{}: {val:?}", prop.name);
/// }
/// # Ok(())
/// # }
/// ```
#[async_trait::async_trait(?Send)]
pub trait Device<P: Read + Write>: private::Sealed {
    /// Connects to the device via the specified port.
    ///
    /// This is an associated function and does not require an existing device instance.
    /// Returns an instance of the device on success.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownSoftwareId`] if the implementation is not compatible
    ///   with the device's software ID.
    ///
    /// See the [`Device`] documentation for other errors.
    async fn connect(port: P) -> Result<Self, P::Error>
    where
        Self: Sized;

    /// Returns the device's software ID.
    fn software_id(&self) -> u16;

    /// Returns the device's kind.
    fn kind(&self) -> DeviceKind;

    /// Returns the set of queryable properties.
    ///
    /// Only properties returned here can be queried via [`Device::query_property`].
    fn properties(&self) -> &'static [Property];

    /// Returns the set of actions that can be triggered.
    ///
    /// Only actions returned here can be triggered via [`Device::trigger_action`].
    fn actions(&self) -> &'static [Action];

    /// Queries a specified property.
    ///
    /// The property must be from the set returned by [`Device::properties`].
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownProperty`] if the device does not support the specified property.
    ///
    /// See the [`Device`] documentation for other errors.
    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error>;

    /// Triggers a specified action.
    ///
    /// The action must be from the set returned by [`Device::actions`].
    ///
    /// Depending on the value of [`Action::params`], the `param` argument
    /// must conform to the specified parameter type.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if `param` does not match the expected type.
    /// - [`Error::UnknownAction`] if the device does not support the specified action.
    ///
    /// See the [`Device`] documentation for other errors.
    async fn trigger_action(
        &mut self,
        action: &Action,
        param: Option<&str>,
    ) -> Result<(), P::Error>;

    /// Returns a mutable reference to the underlying diagnostic interface.
    fn interface(&mut self) -> &mut Interface<P>;
}

/// Connects to a device asynchronously, based on the detected software ID.
///
/// Returns a boxed [`Device`] implementation on success.
///
/// # Errors
///
/// - [`Error::UnknownSoftwareId`] if the device's software ID is not recognized
///   by any supported implementation.
/// - [`Error::Protocol`] for any other errors during diagnostic communication.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut dev = freemdu::device::connect(&mut port).await?;
///
/// println!("{}, software ID {}", dev.kind(), dev.software_id());
/// # Ok(())
/// # }
/// ```
pub async fn connect<'a, P: 'a + Read + Write>(
    port: P,
) -> Result<Box<dyn Device<P> + 'a>, P::Error> {
    let mut intf = Interface::new(port);
    let id = intf.query_software_id().await?;

    match id {
        id132::compatible_software_ids!() => {
            Ok(Box::new(id132::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id218::compatible_software_ids!() => {
            Ok(Box::new(id218::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id324::compatible_software_ids!() => {
            Ok(Box::new(id324::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id360::compatible_software_ids!() => {
            Ok(Box::new(id360::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id419::compatible_software_ids!() => {
            Ok(Box::new(id419::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id469::compatible_software_ids!() => {
            Ok(Box::new(id469::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id605::compatible_software_ids!() => {
            Ok(Box::new(id605::Dishwasher::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id629::compatible_software_ids!() => {
            Ok(Box::new(id629::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id2088::compatible_software_ids!() => {
            Ok(Box::new(id2088::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        id2895::compatible_software_ids!() => {
            Ok(Box::new(id2895::WashingMachine::initialize(intf, id).await?) as Box<dyn Device<P>>)
        }
        _ => Err(Error::UnknownSoftwareId(id)),
    }
}

mod private {
    pub trait Sealed {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::init_logger;
    use alloc::collections::vec_deque::VecDeque;
    use core::convert::Infallible;

    #[tokio::test]
    async fn connect_to_device() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0x75, 0x02, 0x77, 0x00, 0x00, 0x00, 0x00]);

        {
            let dev = connect(&mut deque).await?;

            assert_eq!(dev.software_id(), 629, "software ID should be correct");
            assert_eq!(
                dev.kind(),
                DeviceKind::WashingMachine,
                "device kind should be correct"
            );
        }

        assert_eq!(
            deque,
            [
                0x11, 0x00, 0x00, 0x02, 0x13, 0x00, 0x20, 0xea, 0x43, 0x00, 0x4d, 0x32, 0x02, 0x1f,
                0x00, 0x53, 0x40, 0xc2, 0x02, 0x01, 0x05, 0x01, 0x01,
            ],
            "deque contents should be correct"
        );

        Ok(())
    }

    #[tokio::test]
    async fn error_unknown_software_id() -> Result<(), Infallible> {
        init_logger();

        let mut deque = VecDeque::from([0x00, 0xff, 0xff, 0xfe, 0x00, 0x00]);
        let res = connect(&mut deque).await;

        assert!(
            matches!(res, Err(Error::UnknownSoftwareId(0xffff))),
            "result should be unknown software ID error"
        );

        Ok(())
    }
}
