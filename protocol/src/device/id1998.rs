//! Device support for W 6xx series washing machines.
//!
//! Supports appliances with software ID 1998, which typically use an ELP 165-T KD board or similar.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, Date, Device, DeviceKind, Error, Interface, Property, PropertyKind, Result, Value,
    private,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use bitflags_derive::{FlagsDebug, FlagsDisplay, FlagsFromStr};
use core::{str, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        1998
    };
}
pub(super) use compatible_software_ids;

const PROP_SERIAL_NUMBER: Property = Property {
    kind: PropertyKind::General,
    id: "serial_number",
    name: "Serial Number",
    unit: None,
};
const PROP_SERIAL_NUMBER_INDEX: Property = Property {
    kind: PropertyKind::General,
    id: "serial_number_index",
    name: "Serial Number Index",
    unit: None,
};
const PROP_MODEL_NUMBER: Property = Property {
    kind: PropertyKind::General,
    id: "model_number",
    name: "Model Number",
    unit: None,
};
const PROP_MATERIAL_NUMBER: Property = Property {
    kind: PropertyKind::General,
    id: "material_number",
    name: "Material Number",
    unit: None,
};
const PROP_MANUFACTURING_DATE: Property = Property {
    kind: PropertyKind::General,
    id: "manufacturing_date",
    name: "Manufacturing Date",
    unit: None,
};
const PROP_OPERATING_TIME: Property = Property {
    kind: PropertyKind::General,
    id: "operating_time",
    name: "Operating Time",
    unit: None,
};
const PROP_PROGRAM_TYPE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_type",
    name: "Program Type",
    unit: None,
};
const PROP_PROGRAM_TEMPERATURE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_temperature",
    name: "Program Temperature",
    unit: Some("°C"),
};
const PROP_PROGRAM_OPTIONS: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_options",
    name: "Program Options",
    unit: None,
};
const PROP_PROGRAM_SPIN_SPEED: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_spin_speed",
    name: "Program Spin Speed",
    unit: Some("rpm"),
};
const PROP_LOAD_LEVEL: Property = Property {
    kind: PropertyKind::Operation,
    id: "load_level",
    name: "Load Level",
    unit: None,
};
const PROP_DELAY_START_TIME: Property = Property {
    kind: PropertyKind::Operation,
    id: "delay_start_time",
    name: "Delay Start Time",
    unit: None,
};
const PROP_REMAINING_TIME: Property = Property {
    kind: PropertyKind::Operation,
    id: "remaining_time",
    name: "Remaining Time",
    unit: None,
};
const PROP_TEMPERATURE: Property = Property {
    kind: PropertyKind::Io,
    id: "temperature",
    name: "Temperature",
    unit: Some("°C"),
};
const PROP_MOTOR_SPEED: Property = Property {
    kind: PropertyKind::Io,
    id: "motor_speed",
    name: "Motor Speed",
    unit: Some("rpm"),
};
const PROP_ACTIVE_ACTUATORS: Property = Property {
    kind: PropertyKind::Io,
    id: "active_actuators",
    name: "Active Actuators",
    unit: None,
};
const PROP_ACTIVE_MOTOR_RELAYS: Property = Property {
    kind: PropertyKind::Io,
    id: "active_motor_relays",
    name: "Active Motor Relays",
    unit: None,
};
const PROP_HEATER_RELAY_ACTIVE: Property = Property {
    kind: PropertyKind::Io,
    id: "heater_relay_active",
    name: "Heater Relay Active",
    unit: None,
};

/// Washing program type.
///
/// Represents the general category of a washing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramType {
    /// No program.
    None = 0x00,
    /// Cottons program.
    Cottons = 0x01,
    /// Minimum iron program.
    MinimumIron = 0x03,
    /// Synthetic program.
    Synthetic = 0x05,
    /// Woolens program.
    Woolens = 0x08,
    /// Silks program.
    Silks = 0x09,
    /// Drain/spin program.
    DrainSpin = 0x15,
    /// Shirts program.
    Shirts = 0x17,
    /// Jeans program.
    Jeans = 0x18,
    /// Automatic program.
    Automatic = 0x1f,
    /// Outdoor program.
    Outdoor = 0x25,
    /// Express program.
    Express = 0x31,
    /// Dark garments program.
    DarkGarments = 0x32,
}

bitflags::bitflags! {
    /// Washing program option.
    ///
    /// Each flag represents an optional feature that can be enabled for a program.
    #[derive(FlagsDisplay, FlagsFromStr, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct ProgramOption: u16 {
        /// Soak option enabled.
        const Soak = 0x0001;
        /// Pre-wash option enabled.
        const PreWash = 0x0002;
        /// Water plus option enabled.
        const WaterPlus = 0x0008;
        /// No spin option enabled.
        const NoSpin = 0x0010;
        /// Rinse hold option enabled.
        const RinseHold = 0x0020;
        /// Intensive or short option enabled.
        ///
        /// The actual effect depends on the machine's programming configuration.
        const IntensiveShort = 0x0040;
        /// Extra quiet option enabled.
        const ExtraQuiet = 0x4000;
    }

    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct MotorRelay: u8 {
        const FieldSwitch = 0x10;
        const Reverse = 0x20;
    }

    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u8 {
        const PreWash = 0x01;
        const MainWash = 0x02;
        const Softener = 0x04;
        const DrainPump = 0x08;
        const DoorRelay = 0x10;
    }
}

#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramPhase {
    Idle,
    PreWash,
    Soak,
    PreRinse,
    MainWash,
    Rinse,
    RinseHold,
    Clean,
    Cool,
    Pump,
    Spin,
    AntiCreaseFinish,
    Finish,
}

/// Washing machine device implementation.
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
        intf.unlock_read_access(0x2b67).await?;
        intf.unlock_full_access(0x8235).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the serial number of the machine.
    ///
    /// The serial number consists of 12 digits, e.g. `673528607846`.
    /// It can also be found on the sticker on the back side of the machine's door.
    pub async fn query_serial_number(&mut self) -> Result<String, P::Error> {
        let data: [u8; 12] = self.intf.read_eeprom(0x02e5).await?;
        let serial = str::from_utf8(&data).map_err(|_| Error::UnexpectedMemoryValue)?;

        Ok(serial.to_string())
    }

    /// Queries the serial number index of the machine.
    ///
    /// The serial number index consists of 2 digits, e.g. `03`.
    /// It can also be found on the sticker on the back side of the machine's door.
    pub async fn query_serial_number_index(&mut self) -> Result<String, P::Error> {
        let data: [u8; 2] = self.intf.read_eeprom(0x02ed).await?;
        let idx = str::from_utf8(&data).map_err(|_| Error::UnexpectedMemoryValue)?;

        Ok(idx.to_string())
    }

    /// Queries the model number of the machine.
    ///
    /// The model number has a maximum length of 15 characters, e.g. `W627F`.
    /// It can also be found on the sticker on the back side of the machine's door.
    pub async fn query_model_number(&mut self) -> Result<String, P::Error> {
        let data: [u8; 15] = self.intf.read_eeprom(0x02ef).await?;
        let model = str::from_utf8(&data[1..]).map_err(|_| Error::UnexpectedMemoryValue)?;

        Ok(model.trim_end().to_string())
    }

    /// Queries the material number of the machine.
    ///
    /// The material number consists of 8 digits, e.g. `74353768`.
    /// It can also be found on the sticker on the back side of the machine's door.
    pub async fn query_material_number(&mut self) -> Result<String, P::Error> {
        let data: [u8; 8] = self.intf.read_eeprom(0x02fe).await?;
        let mat = str::from_utf8(&data).map_err(|_| Error::UnexpectedMemoryValue)?;

        Ok(mat.to_string())
    }

    /// Queries the manufacturing/inspection date of the machine.
    pub async fn query_manufacturing_date(&mut self) -> Result<Date, P::Error> {
        let date: [u8; 4] = self.intf.read_eeprom(0x02bc).await?;

        Ok(Date::new(
            u16::from(date[0]) + u16::from(date[1]) * 100,
            date[2],
            date[3],
        ))
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    /// It is internally stored in minutes and hours but only the hours are displayed in the service mode.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        let time: [u8; 5] = self.intf.read_memory(0x1cd2).await?;
        let mins = time[0];
        let hours = u32::from_le_bytes([time[1], time[2], time[3], time[4]]);

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the program type.
    pub async fn query_program_type(&mut self) -> Result<ProgramType, P::Error> {
        ProgramType::from_repr(self.intf.read_memory(0x1d6c).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program temperature.
    ///
    /// The program temperature is provided in `°C` (degrees Celsius).
    pub async fn query_program_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x1d6d).await?)
    }

    /// Queries the program options.
    ///
    /// The program options are typically set using the buttons on the front panel of the machine,
    /// although not all combinations can be selected.
    pub async fn query_program_options(&mut self) -> Result<ProgramOption, P::Error> {
        let opts: u16 = self.intf.read_memory(0x1d6f).await?;

        // The intensive/short option is inverted.
        ProgramOption::from_bits(opts ^ 0x0040).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program spin speed.
    ///
    /// The spin speed is provided in `rpm` (revolutions per minute).
    pub async fn query_program_spin_speed(&mut self) -> Result<u16, P::Error> {
        let speed: u8 = self.intf.read_memory(0x1d6e).await?;

        Ok(u16::from(speed) * 10)
    }

    pub async fn query_load_level(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x1cf0).await?)
    }

    pub async fn query_delay_start_time(&mut self) -> Result<Duration, P::Error> {
        let hours: u8 = self.intf.read_memory(0x1d78).await?;
        let mins: u8 = self.intf.read_memory(0x1d79).await?;

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    pub async fn query_remaining_time(&mut self) -> Result<Duration, P::Error> {
        let hours: u8 = self.intf.read_memory(0x1d7a).await?;
        let mins: u8 = self.intf.read_memory(0x1d7b).await?;

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    pub async fn query_temperature(&mut self) -> Result<(u8, u8), P::Error> {
        let current = self.intf.read_memory(0x0ec1).await?;
        let target = self.intf.read_memory(0x0ecf).await?;

        Ok((current, target))
    }

    pub async fn query_motor_speed(&mut self) -> Result<(u16, u16), P::Error> {
        let current: i16 = self.intf.read_memory(0x0dfd).await?;
        let target: i16 = self.intf.read_memory(0x0dff).await?;

        Ok((current.unsigned_abs() / 10, target.unsigned_abs() / 10))
    }

    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        let actuators: u8 = self.intf.read_memory(0x0f3a).await?;

        Actuator::from_bits(actuators & 0x1f).ok_or(Error::UnexpectedMemoryValue)
    }

    pub async fn query_active_motor_relays(&mut self) -> Result<MotorRelay, P::Error> {
        let relays: u8 = self.intf.read_memory(0x03e0).await?;

        MotorRelay::from_bits(relays & 0x30).ok_or(Error::UnexpectedMemoryValue)
    }

    pub async fn query_heater_relay_active(&mut self) -> Result<bool, P::Error> {
        let state: u8 = self.intf.read_memory(0x0b5d).await?;

        Ok(state != 0x00)
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
            PROP_SERIAL_NUMBER,
            PROP_SERIAL_NUMBER_INDEX,
            PROP_MODEL_NUMBER,
            PROP_MATERIAL_NUMBER,
            PROP_MANUFACTURING_DATE,
            PROP_OPERATING_TIME,
            PROP_PROGRAM_TYPE,
            PROP_PROGRAM_TEMPERATURE,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_SPIN_SPEED,
            PROP_LOAD_LEVEL,
            PROP_DELAY_START_TIME,
            PROP_REMAINING_TIME,
            PROP_TEMPERATURE,
            PROP_MOTOR_SPEED,
            PROP_ACTIVE_ACTUATORS,
            PROP_ACTIVE_MOTOR_RELAYS,
            PROP_HEATER_RELAY_ACTIVE,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_SERIAL_NUMBER => Ok(self.query_serial_number().await?.into()),
            PROP_SERIAL_NUMBER_INDEX => Ok(self.query_serial_number_index().await?.into()),
            PROP_MODEL_NUMBER => Ok(self.query_model_number().await?.into()),
            PROP_MATERIAL_NUMBER => Ok(self.query_material_number().await?.into()),
            PROP_MANUFACTURING_DATE => Ok(self.query_manufacturing_date().await?.into()),
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            // Failure
            // Operation
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_PROGRAM_TEMPERATURE => Ok(self.query_program_temperature().await?.into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_SPIN_SPEED => Ok(self.query_program_spin_speed().await?.into()),
            PROP_LOAD_LEVEL => Ok(self.query_load_level().await?.into()),
            PROP_DELAY_START_TIME => Ok(self.query_delay_start_time().await?.into()),
            PROP_REMAINING_TIME => Ok(self.query_remaining_time().await?.into()),
            // Input/output
            PROP_TEMPERATURE => Ok(self.query_temperature().await?.into()),
            PROP_MOTOR_SPEED => Ok(self.query_motor_speed().await?.into()),
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_ACTIVE_MOTOR_RELAYS => {
                Ok(self.query_active_motor_relays().await?.to_string().into())
            }
            PROP_HEATER_RELAY_ACTIVE => Ok(self.query_heater_relay_active().await?.into()),
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
