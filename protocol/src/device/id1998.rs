//! Device support for W 6xx series washing machines.
//!
//! Supports appliances with software ID 1998, which typically use an ELP 165-T board or similar.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, ActionKind, ActionParameters, Date, Device, DeviceKind, Error, Fault, FaultInfo,
    Interface, Property, PropertyKind, Result, Value,
    common::{
        WashingProgram,
        faults::{self, FaultCode},
    },
    private,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use bitflags_derive::{FlagsDebug, FlagsDisplay, FlagsFromStr};
use core::{str, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr, VariantNames};

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
const PROP_OPERATING_STATE: Property = Property {
    kind: PropertyKind::Operation,
    id: "operating_state",
    name: "Operating State",
    unit: None,
};
const PROP_SELECTED_PROGRAM: Property = Property {
    kind: PropertyKind::Operation,
    id: "selected_program",
    name: "Selected Program",
    unit: None,
};
const PROP_PROGRAM_TEMPERATURE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_temperature",
    name: "Program Temperature",
    unit: Some("°C"),
};
const PROP_PROGRAM_SPIN_SPEED: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_spin_speed",
    name: "Program Spin Speed",
    unit: Some("rpm"),
};
const PROP_PROGRAM_OPTIONS: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_options",
    name: "Program Options",
    unit: None,
};
const PROP_PROGRAM_PHASE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_phase",
    name: "Program Phase",
    unit: None,
};
const PROP_PROGRAM_LOCKED: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_locked",
    name: "Program Locked",
    unit: None,
};
const PROP_LOAD_LEVEL: Property = Property {
    kind: PropertyKind::Operation,
    id: "load_level",
    name: "Load Level",
    unit: None,
};
const PROP_IMBALANCE_MASS: Property = Property {
    kind: PropertyKind::Operation,
    id: "imbalance_mass",
    name: "Imbalance Mass",
    unit: Some("g"),
};
const PROP_IMBALANCE_SPIN_SPEED_LIMIT: Property = Property {
    kind: PropertyKind::Operation,
    id: "imbalance_spin_speed_limit",
    name: "Imbalance Spin Speed Limit",
    unit: Some("rpm"),
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
const PROP_WATER_DIVERTER_POSITION: Property = Property {
    kind: PropertyKind::Io,
    id: "water_diverter_position",
    name: "Water Diverter Position",
    unit: None,
};
const PROP_TEMPERATURE: Property = Property {
    kind: PropertyKind::Io,
    id: "temperature",
    name: "Temperature",
    unit: Some("°C"),
};
const PROP_WATER_LEVEL: Property = Property {
    kind: PropertyKind::Io,
    id: "water_level",
    name: "Water Level",
    unit: Some("mmH₂O"),
};
const PROP_MOTOR_SPEED: Property = Property {
    kind: PropertyKind::Io,
    id: "motor_speed",
    name: "Motor Speed",
    unit: Some("rpm"),
};

const ACTION_SELECT_PROGRAM: Action = Action {
    kind: ActionKind::Operation,
    id: "select_program",
    name: "Select Program",
    params: Some(ActionParameters::Enumeration(WashingProgram::VARIANTS)),
};
const ACTION_SET_PROGRAM_TEMPERATURE: Action = Action {
    kind: ActionKind::Operation,
    id: "set_program_temperature",
    name: "Set Program Temperature",
    params: Some(ActionParameters::Range(0, 85)),
};
const ACTION_SET_PROGRAM_SPIN_SPEED: Action = Action {
    kind: ActionKind::Operation,
    id: "set_program_spin_speed",
    name: "Set Program Spin Speed",
    params: Some(ActionParameters::Range(0, 2000)),
};
const ACTION_SET_PROGRAM_OPTIONS: Action = Action {
    kind: ActionKind::Operation,
    id: "set_program_options",
    name: "Set Program Options",
    params: Some(ActionParameters::Flags(&[
        "Soak",
        "PreWash",
        "WaterPlus",
        "NoSpin",
        "RinseHold",
        "IntensiveShort",
        "RinsePlus",
        "Starch",
        "ExtraQuiet",
    ])),
};
const ACTION_START_PROGRAM: Action = Action {
    kind: ActionKind::Operation,
    id: "start_program",
    name: "Start Program",
    params: None,
};

/// Washing machine operating state.
///
/// Some states can be entered by pressing specific button combinations
/// when turning on the machine.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum OperatingState {
    /// The machine is powered off.
    Off = 0x01,
    /// No program selected.
    Standby = 0x02,
    /// A program has been selected but not started.
    ProgramSelected = 0x03,
    /// Waiting for the program to be started.
    WaitingForStart = 0x04,
    /// A program is currently running.
    Running = 0x05,
    /// The program has been paused.
    Paused = 0x06,
    /// The program completed successfully.
    Finished = 0x07,
    /// A fault has occurred.
    Failure = 0x08,
    /// The program was interrupted.
    Interrupted = 0x09,
    /// The machine is idle.
    Idle = 0x0a,
    /// Program stopped at rinse hold.
    RinseHold = 0x0b,
    /// Demonstration mode for trade fairs or events. Cycles through LEDs and
    /// displays timing for washing program phases.
    ///
    /// Entered by holding the _Start_ button when turning on the machine,
    /// pressing it once and holding it again after the last press.
    Demo = 0x0c,
    /// Unknown state (0x0d).
    Unknown0d = 0x0d,
    /// Service mode. Allows viewing stored faults and testing actuators.
    ///
    /// Entered by holding the _Start_ button when turning on the machine,
    /// pressing it 3 times and holding it again after the last press.
    Service = 0x0e,
    /// Unknown state (0x0f).
    Unknown0f = 0x0f,
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
        /// Rinse plus option enabled.
        const RinsePlus = 0x0080;
        /// Starch option enabled.
        const Starch = 0x0100;
        /// Extra quiet option enabled.
        const ExtraQuiet = 0x4000;
    }
}

/// Washing program phase.
///
/// Phases may not always execute in the defined order and some phases
/// may be skipped depending on the selected washing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramPhase {
    /// Program has not started yet.
    Idle,
    /// Pre-wash phase.
    PreWash,
    /// Soak phase.
    Soak,
    /// Pre-rinse phase.
    PreRinse,
    /// Main wash phase.
    MainWash,
    /// Rinse phase.
    Rinse,
    /// Rinse hold phase.
    RinseHold,
    /// Cleaning phase.
    Cleaning,
    /// Cooling down phase.
    CoolingDown,
    /// Drain phase.
    Drain,
    /// Spin phase.
    Spin,
    /// Anti-crease/finish phase.
    AntiCreaseFinish,
    /// Finish phase.
    Finish,
    /// Venting phase.
    Venting,
    /// Starch stop phase.
    StarchStop,
    /// Freshen-up/moisten phase.
    FreshenUpMoisten,
    /// Steam smoothing phase.
    SteamSmoothing,
    /// Hygiene phase.
    Hygiene,
}

bitflags::bitflags! {
    /// Washing machine actuator.
    ///
    /// Each flag represents a controllable component of the washing machine.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u8 {
        /// Pre-wash valve actuator.
        const PreWash = 0x01;
        /// Main wash valve actuator.
        const MainWash = 0x02;
        /// Softener valve actuator.
        const Softener = 0x04;
        /// Drain pump actuator.
        const DrainPump = 0x08;
        /// Drain relay actuator.
        const DoorRelay = 0x10;
    }

    /// Washing machine motor relay.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct MotorRelay: u8 {
        /// Field switch relay.
        const FieldSwitch = 0x10;
        /// Reverse relay.
        const Reverse = 0x20;
    }
}

/// Water diverter position.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum WaterDiverterPosition {
    /// Unknown position (diverter is moving).
    Unknown,
    /// Door glass position.
    DoorGlass,
    /// Pre-wash compartment position.
    PreWash,
    /// Main wash compartment position.
    MainWash,
    /// Softener compartment position.
    Softener,
}

/// Washing machine device implementation.
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
/// println!("Model number: {}", machine.query_model_number().await?);
/// println!("Selected program: {}", machine.query_selected_program().await?);
/// println!("Program options: {}", machine.query_program_options().await?);
///
/// machine.start_program().await?;
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

    /// Queries the status of a fault identified by its fault code.
    ///
    /// Faults may be either currently active or stored persistently in EEPROM
    /// from a previous occurrence when the machine was powered off.
    /// Returned faults include operating hours and occurrence count information.
    pub async fn query_fault(&mut self, code: FaultCode) -> Result<Fault, P::Error> {
        // Each fault occupies 3 bytes in the fault table
        let table_idx: u16 = match code {
            FaultCode::NtcThermistorShortWater => 1,
            FaultCode::NtcThermistorOpenWater => 2,
            FaultCode::NtcThermistorShortAir => 3,
            FaultCode::NtcThermistorOpenAir => 4,
            FaultCode::ColdWaterInlet => 5,
            FaultCode::Drainage => 6,
            FaultCode::HotWaterInlet => 7,
            FaultCode::DetergentOverdose => 8,
            // Entry 9 stores F18, which doesn't exist on washing machines
            FaultCode::FlowMeter => 10,
            FaultCode::Heater => 11,
            FaultCode::DoorLocking => 12,
            FaultCode::DoorUnlocking => 13,
            FaultCode::ControlElectronics => 14,
            FaultCode::Eeprom => 15,
            FaultCode::DeviceType => 16,
            FaultCode::FlashRam => 17,
            FaultCode::Display => 18,
            FaultCode::BoardInterface => 19,
            FaultCode::AuxiliaryRelayBoard => 20,
            FaultCode::DrumMotor => 21,
            FaultCode::PressureSensor => 22,
            FaultCode::Tachometer => 23,
            FaultCode::TimeoutDryer => 24,
            FaultCode::FinalSpinSpeed => 25,
            FaultCode::ProgramSelector => 26,
            FaultCode::WaterDiverter => 27,
            FaultCode::LoadSensor => 28,
            FaultCode::DrumLightCap => 29,
            FaultCode::HygieneInfo => 30,
            FaultCode::AuxiliaryRelayBoardAlt => 31,
            FaultCode::Ik6Communication => 32,
            FaultCode::Ik6DefectiveIncompatible => 33,
            FaultCode::SmartHomeCommunication => 34,
            FaultCode::SmartHomeIncompatible => 35,
            FaultCode::GrayWaterInlet => 36,
            FaultCode::EzlCommunication => 37,
            FaultCode::EzlDefectiveIncompatible => 38,
            FaultCode::DrumMotorLowVoltage => 39,
            FaultCode::WpsDispenser => 40,
            FaultCode::DrainageDispenser => 41,
            FaultCode::SteamInactive => 42,
            FaultCode::SteamExcessiveTemperature => 43,
            FaultCode::NtcThermistorShortSteam => 44,
            FaultCode::NtcThermistorOpenSteam => 45,
            FaultCode::DripTrayWater => 46,
            _ => return Err(Error::InvalidArgument),
        };

        let count_addr = 0x0a3b + table_idx * 3;
        let hours_addr = count_addr + 1;

        // Active faults are stored as individual bits
        let active_addr = 0x0af8 + (code as u16) / 8;
        let active_bit = 7 - (code as u16 % 8);

        let count: u8 = self.intf.read_memory(count_addr.into()).await?;

        if count == 0x00 {
            return Ok(Fault::Ok);
        }

        let hours: u16 = self.intf.read_memory(hours_addr.into()).await?;
        let info = FaultInfo {
            operating_hours: hours.into(),
            count: count.into(),
        };

        let active: u8 = self.intf.read_memory(active_addr.into()).await?;

        if (active & (1 << active_bit)) == 0x00 {
            Ok(Fault::Stored(Some(info)))
        } else {
            Ok(Fault::Active(Some(info)))
        }
    }

    /// Queries the operating state.
    pub async fn query_operating_state(&mut self) -> Result<OperatingState, P::Error> {
        OperatingState::from_repr(self.intf.read_memory(0x1d66).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the selected program.
    pub async fn query_selected_program(&mut self) -> Result<WashingProgram, P::Error> {
        WashingProgram::from_repr(self.intf.read_memory(0x1d6c).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Selects a program.
    ///
    /// The machine will show an error if the requested program does not exist.
    pub async fn select_program(&mut self, prog: WashingProgram) -> Result<(), P::Error> {
        self.change_program(|data| {
            data[0] = 0x02;
            data[1] = prog as u8;
            data[2..].fill(0x00);
        })
        .await
    }

    /// Queries the program temperature.
    ///
    /// The program temperature is provided in `°C` (degrees Celsius).
    pub async fn query_program_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x1d6d).await?)
    }

    /// Sets the program temperature.
    pub async fn set_program_temperature(&mut self, temp: u8) -> Result<(), P::Error> {
        self.change_program(|data| {
            data[0] = 0x01;
            data[2] = temp;
        })
        .await
    }

    /// Queries the program spin speed.
    ///
    /// The spin speed is provided in `rpm` (revolutions per minute).
    pub async fn query_program_spin_speed(&mut self) -> Result<u16, P::Error> {
        let speed: u8 = self.intf.read_memory(0x1d6e).await?;

        Ok(u16::from(speed) * 10)
    }

    /// Sets the program spin speed.
    ///
    /// Selecting a spin speed higher than the maximum speed set in the machine's programing options
    /// will have no effect.
    /// The allowed spin speeds depend on the machine's programming options.
    pub async fn set_program_spin_speed(&mut self, speed: u16) -> Result<(), P::Error> {
        let speed = u8::try_from(speed / 10).map_err(|_| Error::UnexpectedMemoryValue)?;

        self.change_program(|data| {
            data[0] = 0x01;
            data[3] = speed;
        })
        .await
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

    /// Sets the program options.
    pub async fn set_program_options(&mut self, opts: ProgramOption) -> Result<(), P::Error> {
        let opts = (opts.bits() ^ 0x0040).to_le_bytes();

        self.change_program(|data| {
            data[0] = 0x01;
            data[4..6].copy_from_slice(&opts);
        })
        .await
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        ProgramPhase::from_repr(self.intf.read_memory(0x1d76).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program locked state.
    ///
    /// The currently running program can be locked/unlocked by holding the _Start_ button.
    pub async fn query_program_locked(&mut self) -> Result<bool, P::Error> {
        let state: u8 = self.intf.read_memory(0x1cf7).await?;

        Ok(state != 0x00)
    }

    /// Queries the laundry load level.
    ///
    /// The load level ranges from 1 to 4 and is calculated by the machine during operation.
    /// For some program types, the maximum load level is limited to a lower value.
    pub async fn query_load_level(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x1cf0).await?)
    }

    /// Queries the imbalance mass sensed by the machine.
    ///
    /// The mass is provided in `g` (gram).
    pub async fn query_imbalance_mass(&mut self) -> Result<u16, P::Error> {
        let mass: u8 = self.intf.read_memory(0x0ec5).await?;

        Ok(u16::from(mass) * 100)
    }

    /// Queries the motor speed spin limit due to imbalance.
    ///
    /// The speed limit is provided in `rpm` (revolutions per minute)
    /// and is calculated by the machine based on the determined imbalance.
    pub async fn query_imbalance_spin_speed_limit(&mut self) -> Result<u8, P::Error> {
        let speed: u8 = self.intf.read_memory(0x0df9).await?;

        Ok(speed * 10)
    }

    /// Queries the remaining time until the program starts, if delay start is enabled.
    pub async fn query_delay_start_time(&mut self) -> Result<Duration, P::Error> {
        let hours: u8 = self.intf.read_memory(0x1d78).await?;
        let mins: u8 = self.intf.read_memory(0x1d79).await?;

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the remaining time of the active program.
    pub async fn query_remaining_time(&mut self) -> Result<Duration, P::Error> {
        let hours: u8 = self.intf.read_memory(0x1d7a).await?;
        let mins: u8 = self.intf.read_memory(0x1d7b).await?;

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        let actuators: u8 = self.intf.read_memory(0x0f3a).await?;

        Actuator::from_bits(actuators & 0x1f).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the currently active motor relays.
    pub async fn query_active_motor_relays(&mut self) -> Result<MotorRelay, P::Error> {
        // The relay state is directly read from port 0.
        let relays: u8 = self.intf.read_memory(0x03e0).await?;

        MotorRelay::from_bits(relays & 0x30).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the current state of the heater relay (on/off).
    pub async fn query_heater_relay_active(&mut self) -> Result<bool, P::Error> {
        let state: u8 = self.intf.read_memory(0x0b5d).await?;

        Ok(state != 0x00)
    }

    /// Queries the current water diverter position.
    pub async fn query_water_diverter_position(
        &mut self,
    ) -> Result<WaterDiverterPosition, P::Error> {
        WaterDiverterPosition::from_repr(self.intf.read_memory(0x1ce6).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the current temperature sensed by the NTC thermistor and the target temperature.
    ///
    /// The temperature is provided in `°C` (degrees Celsius).
    pub async fn query_temperature(&mut self) -> Result<(u8, u8), P::Error> {
        let current = self.intf.read_memory(0x0ec1).await?;
        let target = self.intf.read_memory(0x0ecf).await?;

        Ok((current, target))
    }

    /// Queries the current water level sensed by the analog pressure sensor and the target level.
    ///
    /// The water level is provided in `mmH₂O` (millimeters of water).
    pub async fn query_water_level(&mut self) -> Result<(u16, u16), P::Error> {
        let current: i16 = self.intf.read_memory(0x06ad).await?;
        let target: i16 = self.intf.read_memory(0x086c).await?;

        // Clamp negative values.
        Ok((
            current.max(0).unsigned_abs() / 10,
            target.max(0).unsigned_abs() / 10,
        ))
    }

    /// Queries the current and target motor speed.
    ///
    /// The speed is provided in `rpm` (revolutions per minute).
    pub async fn query_motor_speed(&mut self) -> Result<(u16, u16), P::Error> {
        let current: i16 = self.intf.read_memory(0x0dff).await?;
        let target: i16 = self.intf.read_memory(0x0e01).await?;

        Ok((current.unsigned_abs() / 10, target.unsigned_abs() / 10))
    }

    /// Starts the selected program.
    ///
    /// This function returns an error if no program has been chosen
    /// or a program is already running.
    pub async fn start_program(&mut self) -> Result<(), P::Error> {
        if self.query_operating_state().await? == OperatingState::ProgramSelected {
            Ok(self.intf.write_memory(0x08de, 0x02u8).await?)
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Stops the currently running program.
    pub async fn stop_program(&mut self) -> Result<(), P::Error> {
        Ok(self.intf.write_memory(0x08de, 0x01u8).await?)
    }

    /// Changes the selected program or its options.
    async fn change_program(&mut self, modify: impl FnOnce(&mut [u8; 9])) -> Result<(), P::Error> {
        const PROP_ID: u8 = 0x03;

        // Read "selected program" control board property
        let mut prop: [u8; 9] = self.intf.read_memory(0x1f48).await?;

        modify(&mut prop);

        // Write "requested program" control board property
        self.intf.write_memory(0x1dc3, prop).await?;

        // Notify control board that property has changed
        self.intf
            .write_memory(0x0c21 + u32::from(PROP_ID) * 0x0a, 0x01)
            .await?;
        self.intf
            .write_memory(0x0d01 + u32::from(PROP_ID) * 0xd0, 0x01)
            .await?;

        Ok(())
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
            PROP_OPERATING_STATE,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_TEMPERATURE,
            PROP_PROGRAM_SPIN_SPEED,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_PHASE,
            PROP_PROGRAM_LOCKED,
            PROP_LOAD_LEVEL,
            PROP_IMBALANCE_MASS,
            PROP_IMBALANCE_SPIN_SPEED_LIMIT,
            PROP_DELAY_START_TIME,
            PROP_REMAINING_TIME,
            PROP_ACTIVE_ACTUATORS,
            PROP_ACTIVE_MOTOR_RELAYS,
            PROP_HEATER_RELAY_ACTIVE,
            PROP_WATER_DIVERTER_POSITION,
            PROP_TEMPERATURE,
            PROP_WATER_LEVEL,
            PROP_MOTOR_SPEED,
            faults::PROP_FAULT_F1,
            faults::PROP_FAULT_F2,
            faults::PROP_FAULT_F3,
            faults::PROP_FAULT_F4,
            faults::PROP_FAULT_F10,
            faults::PROP_FAULT_F11,
            faults::PROP_FAULT_F15,
            faults::PROP_FAULT_F16,
            faults::PROP_FAULT_F19,
            faults::PROP_FAULT_F20,
            faults::PROP_FAULT_F34,
            faults::PROP_FAULT_F35,
            faults::PROP_FAULT_F39,
            faults::PROP_FAULT_F41,
            faults::PROP_FAULT_F43,
            faults::PROP_FAULT_F45,
            faults::PROP_FAULT_F46,
            faults::PROP_FAULT_F47,
            faults::PROP_FAULT_F49,
            faults::PROP_FAULT_F50,
            faults::PROP_FAULT_F51,
            faults::PROP_FAULT_F53,
            faults::PROP_FAULT_F55,
            faults::PROP_FAULT_F56,
            faults::PROP_FAULT_F62,
            faults::PROP_FAULT_F63,
            faults::PROP_FAULT_F64,
            faults::PROP_FAULT_F65,
            faults::PROP_FAULT_F81,
            faults::PROP_FAULT_F83,
            faults::PROP_FAULT_F92,
            faults::PROP_FAULT_F93,
            faults::PROP_FAULT_F96,
            faults::PROP_FAULT_F100,
            faults::PROP_FAULT_F101,
            faults::PROP_FAULT_F102,
            faults::PROP_FAULT_F103,
            faults::PROP_FAULT_F104,
            faults::PROP_FAULT_F105,
            faults::PROP_FAULT_F106,
            faults::PROP_FAULT_F130,
            faults::PROP_FAULT_F131,
            faults::PROP_FAULT_F138,
            faults::PROP_FAULT_F139,
            faults::PROP_FAULT_F140,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[
            ACTION_SELECT_PROGRAM,
            ACTION_SET_PROGRAM_TEMPERATURE,
            ACTION_SET_PROGRAM_SPIN_SPEED,
            ACTION_SET_PROGRAM_OPTIONS,
            ACTION_START_PROGRAM,
        ]
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
            // Operation
            PROP_OPERATING_STATE => Ok(self.query_operating_state().await?.to_string().into()),
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_TEMPERATURE => Ok(self.query_program_temperature().await?.into()),
            PROP_PROGRAM_SPIN_SPEED => Ok(self.query_program_spin_speed().await?.into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_PROGRAM_LOCKED => Ok(self.query_program_locked().await?.into()),
            PROP_LOAD_LEVEL => Ok(self.query_load_level().await?.into()),
            PROP_IMBALANCE_MASS => Ok(self.query_imbalance_mass().await?.into()),
            PROP_IMBALANCE_SPIN_SPEED_LIMIT => {
                Ok(self.query_imbalance_spin_speed_limit().await?.into())
            }
            PROP_DELAY_START_TIME => Ok(self.query_delay_start_time().await?.into()),
            PROP_REMAINING_TIME => Ok(self.query_remaining_time().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_ACTIVE_MOTOR_RELAYS => {
                Ok(self.query_active_motor_relays().await?.to_string().into())
            }
            PROP_HEATER_RELAY_ACTIVE => Ok(self.query_heater_relay_active().await?.into()),
            PROP_WATER_DIVERTER_POSITION => Ok(self
                .query_water_diverter_position()
                .await?
                .to_string()
                .into()),
            PROP_TEMPERATURE => Ok(self.query_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
            PROP_MOTOR_SPEED => Ok(self.query_motor_speed().await?.into()),
            _ => {
                if let Some(code) = faults::prop_to_fault_code(prop) {
                    Ok(self.query_fault(code).await?.into())
                } else {
                    Err(Error::UnknownProperty)
                }
            }
        }
    }

    async fn trigger_action(
        &mut self,
        action: &Action,
        param: Option<&str>,
    ) -> Result<(), P::Error> {
        match *action {
            ACTION_SELECT_PROGRAM => match param {
                Some(s) => self.select_program(s.parse()?).await,
                None => Err(Error::InvalidArgument),
            },
            ACTION_SET_PROGRAM_TEMPERATURE => match param {
                Some(s) => self.set_program_temperature(s.parse()?).await,
                None => Err(Error::InvalidArgument),
            },
            ACTION_SET_PROGRAM_SPIN_SPEED => match param {
                Some(s) => self.set_program_spin_speed(s.parse()?).await,
                None => Err(Error::InvalidArgument),
            },
            ACTION_SET_PROGRAM_OPTIONS => match param {
                Some(s) => self.set_program_options(s.parse()?).await,
                None => Err(Error::InvalidArgument),
            },
            ACTION_START_PROGRAM => match param {
                None => self.start_program().await,
                Some(_) => Err(Error::InvalidArgument),
            },
            _ => Err(Error::UnknownAction),
        }
    }
}

impl<P> private::Sealed for WashingMachine<P> {}
