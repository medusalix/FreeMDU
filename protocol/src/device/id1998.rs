//! Device support for W 6xx series washing machines.
//!
//! Supports appliances with software ID 1998, such as the W627F.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.
//!
//! # Note
//!
//! This is a stub implementation. Memory addresses for properties need to be
//! discovered by dumping and analyzing the device's memory and EEPROM.

use crate::device::{
    Action, Device, DeviceKind, Error, Interface, Property, PropertyKind, Result, Value, private,
    utils,
};
use alloc::{boxed::Box, string::ToString};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::time::Duration;
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        1998
    };
}
pub(super) use compatible_software_ids;

const PROP_ROM_CODE: Property = Property {
    kind: PropertyKind::General,
    id: "rom_code",
    name: "ROM Code",
    unit: None,
};
const PROP_OPERATING_TIME: Property = Property {
    kind: PropertyKind::General,
    id: "operating_time",
    name: "Operating Time",
    unit: None,
};
const PROP_FAULTS: Property = Property {
    kind: PropertyKind::Failure,
    id: "faults",
    name: "Faults",
    unit: None,
};
const PROP_OPERATING_MODE: Property = Property {
    kind: PropertyKind::Operation,
    id: "operating_mode",
    name: "Operating Mode",
    unit: None,
};

bitflags::bitflags! {
    /// Washing machine fault.
    ///
    /// Each flag represents a specific fault condition that can occur in the machine.
    /// Multiple faults may be active simultaneously.
    ///
    /// Note: These fault flags are placeholders and need to be verified by
    /// analyzing the device's memory.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Fault: u8 {
        /// Analog pressure sensor fault detected.
        const PressureSensor = 0x01;
        /// NTC thermistor (temperature sensor) fault detected.
        const NtcThermistor = 0x02;
        /// Heater fault detected.
        const Heater = 0x04;
        /// Tachometer generator fault detected.
        const TachometerGenerator = 0x08;
        /// Detergent overdose fault detected.
        const DetergentOverdose = 0x10;
        /// Inlet fault detected.
        const Inlet = 0x20;
        /// Drainage fault detected.
        const Drainage = 0x40;
        /// EEPROM fault detected.
        const Eeprom = 0x80;
    }
}

/// Washing machine operating mode.
///
/// Note: These modes are placeholders and need to be verified by
/// analyzing the device's memory.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum OperatingMode {
    /// Default mode when the machine is turned on.
    ProgramIdle = 0x01,
    /// A washing program is currently running.
    ProgramRunning = 0x02,
    /// The washing program has finished.
    ProgramFinished = 0x03,
    /// Service programming mode.
    ServiceProgramming = 0x04,
    /// Service mode.
    Service = 0x05,
    /// Customer programming mode.
    CustomerProgramming = 0x06,
}

/// Washing machine device implementation for W627F (Software ID 1998).
///
/// Connect to a compatible washing machine using [`WashingMachine::connect`].
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// use freemdu::device::{Device, id1998::WashingMachine};
///
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut machine = WashingMachine::connect(&mut port).await?;
///
/// // Dump memory to discover addresses
/// for addr in (0..=0xffff).step_by(0x80) {
///     let data: [u8; 0x80] = machine.interface().read_memory(addr).await?;
///     // Process data...
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct WashingMachine<P> {
    intf: Interface<P>,
    software_id: u16,
}

impl<P: Read + Write> WashingMachine<P> {
    pub(crate) async fn initialize(
        mut intf: Interface<P>,
        software_id: u16,
    ) -> Result<Self, P::Error> {
        // Legacy protocol requires dummy bytes (like id419)
        intf.enable_dummy_bytes().await?;

        // Keys provided by user for W627F
        // Read Key: 0x2b67
        // Full Access Key: 0x8235
        intf.unlock_read_access(0x2b67).await?;
        intf.unlock_full_access(0x8235).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the ROM code of the machine's microcontroller.
    ///
    /// Note: The address 0xffdf is a placeholder based on id419.
    /// The actual address may differ for this device.
    pub async fn query_rom_code(&mut self) -> Result<u8, P::Error> {
        // Address based on id419, may need adjustment
        Ok(self.intf.read_memory(0xffdf).await?)
    }

    /// Queries the total operating time of the machine.
    ///
    /// Note: The address is a placeholder based on id419.
    /// The actual address may differ for this device.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // Address based on id419, may need adjustment
        let time: u32 = self.intf.read_memory(0x0014).await?;
        let mins = time & 0x0000_00ff;
        let hours = utils::decode_bcd_value((time & 0xffff_ff00) >> 8);

        Ok(Duration::from_secs(u64::from(hours * 60 * 60 + mins * 60)))
    }

    /// Queries the stored faults.
    ///
    /// Note: The address is a placeholder based on id419.
    /// The actual address may differ for this device.
    pub async fn query_faults(&mut self) -> Result<Fault, P::Error> {
        // Address based on id419, may need adjustment
        Fault::from_bits(self.intf.read_memory(0x000e).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the operating mode.
    ///
    /// Note: The address is a placeholder based on id419.
    /// The actual address may differ for this device.
    pub async fn query_operating_mode(&mut self) -> Result<OperatingMode, P::Error> {
        // Address based on id419, may need adjustment
        OperatingMode::from_repr(self.intf.read_memory(0x0089).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }
}

#[async_trait::async_trait(?Send)]
impl<P: Read + Write> Device<P> for WashingMachine<P> {
    async fn connect(port: P) -> Result<Self, P::Error> {
        let mut intf = Interface::new(port);
        let id = intf.query_software_id().await?;

        match id {
            compatible_software_ids!() => Self::initialize(intf, id).await,
            _ => Err(Error::UnknownSoftwareId(id)),
        }
    }

    fn interface(&mut self) -> &mut Interface<P> {
        &mut self.intf
    }

    fn software_id(&self) -> u16 {
        self.software_id
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::WashingMachine
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_ROM_CODE,
            PROP_OPERATING_TIME,
            PROP_FAULTS,
            PROP_OPERATING_MODE,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        // No actions implemented yet - need to discover memory addresses first
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            PROP_ROM_CODE => Ok(self.query_rom_code().await?.into()),
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            PROP_FAULTS => Ok(self.query_faults().await?.to_string().into()),
            PROP_OPERATING_MODE => Ok(self.query_operating_mode().await?.to_string().into()),
            _ => Err(Error::UnknownProperty),
        }
    }

    async fn trigger_action(
        &mut self,
        _action: &Action,
        _param: Option<Value>,
    ) -> Result<(), P::Error> {
        Err(Error::UnknownAction)
    }
}

impl<P> private::Sealed for WashingMachine<P> {}
