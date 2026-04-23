//! Device support for G 7xxx series glasswashers.
//!
//! Supports appliances with software ID 517, which typically use an EGPL 061 board or similar.
//!
//! A glasswasher instance can be obtained using [`Glasswasher::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, ActionKind, Device, DeviceKind, Error, Fault, Interface, Property, PropertyKind,
    Result, Value, private, utils,
};
use alloc::{boxed::Box, string::ToString};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::{str, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        517
    };
}
pub(super) use compatible_software_ids;

const PROP_MACHINE_NUMBER: Property = Property {
    kind: PropertyKind::General,
    id: "machine_number",
    name: "Machine Number",
    unit: None,
};
const PROP_FAULT_F1: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f1",
    name: "F1: Temperature Main Wash",
    unit: None,
};
const PROP_FAULT_F2: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f2",
    name: "F2: Temperature Final Rinse",
    unit: None,
};
const PROP_FAULT_F4: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f4",
    name: "F4: NTC Thermistor Open",
    unit: None,
};
const PROP_FAULT_F5: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f5",
    name: "F5: NTC Thermistor Short",
    unit: None,
};
const PROP_FAULT_F8: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f8",
    name: "F8: Speed Sensor",
    unit: None,
};
const PROP_FAULT_F9: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f9",
    name: "F9: Mains Frequency",
    unit: None,
};
const PROP_FAULT_F10: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f10",
    name: "F10: Program Selector",
    unit: None,
};
const PROP_FAULT_F11: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f11",
    name: "F11: External Module I²C",
    unit: None,
};
const PROP_FAULT_F23: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f23",
    name: "F23: Insufficient Salt",
    unit: None,
};
const PROP_FAULT_F24: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f24",
    name: "F24: Motor Triac",
    unit: None,
};
const PROP_FAULT_F25: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f25",
    name: "F25: Pressure Switch Start",
    unit: None,
};
const PROP_FAULT_F26: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f26",
    name: "F26: Water Overflow",
    unit: None,
};
const PROP_FAULT_FA: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_fa",
    name: "FA: Pressure Switch Draining",
    unit: None,
};
const PROP_FAULT_F0E: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f0e",
    name: "F0E: Water Inlet Start",
    unit: None,
};
const PROP_FAULT_F1E: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f1e",
    name: "F1E: Water Inlet End",
    unit: None,
};
const PROP_FAULT_F2E: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f2e",
    name: "F2E: Water Inlet Duration",
    unit: None,
};
const PROP_FAULT_F3E: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f3e",
    name: "F3E: Water Inlet Pressure",
    unit: None,
};
const PROP_FAULT_F4E: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f4e",
    name: "F4E: Water Inlet Inadvertent",
    unit: None,
};
const PROP_SELECTED_PROGRAM: Property = Property {
    kind: PropertyKind::Operation,
    id: "selected_program",
    name: "Selected Program",
    unit: None,
};
const PROP_PROGRAM_TYPE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_type",
    name: "Program Type",
    unit: None,
};
const PROP_DRYING_ENABLED: Property = Property {
    kind: PropertyKind::Operation,
    id: "drying_enabled",
    name: "Drying Enabled",
    unit: None,
};
const PROP_PROGRAM_PHASE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_phase",
    name: "Program Phase",
    unit: None,
};
const PROP_PROGRAM_STEP: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_step",
    name: "Program Step",
    unit: None,
};
const PROP_PROGRAM_ELAPSED_TIME: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_elapsed_time",
    name: "Program Elapsed Time",
    unit: None,
};
const PROP_ACTIVE_ACTUATORS: Property = Property {
    kind: PropertyKind::Io,
    id: "active_actuators",
    name: "Active Actuators",
    unit: None,
};
const PROP_CLOSED_SWITCHES: Property = Property {
    kind: PropertyKind::Io,
    id: "closed_switches",
    name: "Closed Switches",
    unit: None,
};
const PROP_NTC_RESISTANCE: Property = Property {
    kind: PropertyKind::Io,
    id: "ntc_resistance",
    name: "NTC Resistance",
    unit: Some("Ω"),
};
const PROP_FLOW_METER_PULSES: Property = Property {
    kind: PropertyKind::Io,
    id: "flow_meter_pulses",
    name: "Flow Meter Pulses",
    unit: None,
};

const ACTION_START_PROGRAM: Action = Action {
    kind: ActionKind::Operation,
    id: "start_program",
    name: "Start Program",
    params: None,
};

/// Glasswasher fault code.
///
/// Each code represents a specific fault condition that can occur in the machine.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum FaultCode {
    /// Main wash water temperature T1 not reached within 1 hour.
    TemperatureMainWash = 1,
    /// Final rinse water temperature T2 not reached within 1 hour.
    TemperatureFinalRinse = 2,
    /// NTC thermistor (temperature sensor) open circuit fault.
    NtcThermistorOpen = 4,
    /// NTC thermistor (temperature sensor) short circuit fault.
    NtcThermistorShort = 5,
    /// Circulation pump speed sensor fault.
    SpeedSensor = 8,
    /// Mains frequency not detected.
    MainsFrequency = 9,
    /// Program selector knob fault.
    ProgramSelector = 10,
    /// External module I²C bus fault.
    ExternalModuleI2C = 11,
    /// Insufficient salt.
    InsufficientSalt = 23,
    /// Circulation pump motor triac fault.
    MotorTriac = 24,
    /// Heater pressure switch activated before program start.
    PressureSwitchStart = 25,
    /// Water overflow.
    WaterOverflow,
    /// Heater pressure switch activated after draining.
    PressureSwitchDraining,
    /// Water inlet fault at the start of the inlet phase.
    WaterInletStart,
    /// Water inlet fault at the end of the inlet phase.
    WaterInletEnd,
    /// Water inlet phase not completed after 5 minutes.
    WaterInletDuration,
    /// Heater pressure switch not activated during water inlet phase.
    WaterInletPressure,
    /// Inadvertent water inlet activation.
    WaterInletInadvertent,
}

/// Glasswashing program.
///
/// Each variant represents a position of the machine's program selector knob.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Program {
    /// Stop position (no program selected).
    Stop,
    /// Short program (A).
    ShortA,
    /// Regular program (B).
    RegularB,
    /// Extended program (C).
    ExtendedC,
    /// No program (4 o'clock).
    None1,
    /// No program (5 o'clock).
    None2,
    /// No program (6 o'clock).
    None3,
    /// No program (7 o'clock).
    None4,
    /// No program (8 o'clock).
    None5,
    /// No program (9 o'clock).
    None6,
    /// Drain program.
    Drain,
    /// Rinse program.
    Rinse,
}

/// Glasswashing program type.
///
/// Represents the general category of a glasswashing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramType {
    /// No program.
    None = 0x00,
    /// Short program (A).
    ShortA = 0x01,
    /// Regular program (B).
    RegularB = 0x02,
    /// Extended program (C).
    ExtendedC = 0x03,
    /// Rinse program.
    Rinse = 0x04,
    /// Drain program.
    Drain = 0x05,
    /// Test program.
    ///
    /// Only available in service mode 1.
    Test = 0x06,
    /// Invalid program.
    Invalid = 0xff,
}

/// Glasswashing program phase.
///
/// Some phases may be skipped depending on the selected washing program.
#[derive(Display, PartialEq, Eq, Copy, Clone, Debug)]
pub enum ProgramPhase {
    /// Program has not started yet.
    Idle,
    /// Pre-rinse phase.
    PreRinse,
    /// Pre-wash phase.
    PreWash,
    /// Main wash phase.
    MainWash,
    /// First interim rinse phase.
    InterimRinse1,
    /// Second interim rinse phase.
    InterimRinse2,
    /// Final rinse phase.
    FinalRinse,
    /// Drying phase.
    Drying,
    /// Program has finished.
    Finish,
}

bitflags::bitflags! {
    /// Glasswasher actuator.
    ///
    /// Each flag represents a controllable component of the glasswasher.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u32 {
        /// Circulation pump actuator.
        const CirculationPump = 0x0000_0080;
        /// Drain pump actuator.
        const DrainPump = 0x0000_0100;
        /// Cold water inlet actuator.
        const ColdWaterInlet = 0x0000_0200;
        /// Reactivation actuator.
        const Reactivation = 0x0000_0400;
        /// Detergent dosing actuator (door/external).
        const Dos1 = 0x0000_0800;
        /// Neutralizer/rinse aid dosing actuator (door).
        const DuoDos = 0x0000_1000;
        /// Door lock actuator.
        const DoorLock = 0x0000_2000;
        /// Purified water inlet actuator.
        const PurifiedWaterInlet = 0x0000_4000;
        /// Heater actuator.
        const Heater = 0x0020_0000;
        /// Dryer actuator.
        const Dryer = 0x0040_0000;
        /// Neutralizer dosing actuator (external).
        const Dos3 = 0x0080_0000;
    }
}

bitflags::bitflags! {
    /// Glasswasher switch status.
    ///
    /// Each flag corresponds to a switch signal from the glasswasher's sensors.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Switch: u8 {
        /// Heater pressure switch.
        ///
        /// This switch closes when the water pressure reaches a certain threshold
        /// while the circulation pump is running, allowing the heater to be activated.
        const HeaterPressure = 0x01;
        /// Salt reservoir reed switch.
        ///
        /// This switch closes when salt is present in the reservoir.
        const SaltPresent = 0x04;
        /// Detergent reservoir switch.
        ///
        /// This switch closes when detergent is present in
        /// the door-mounted reservoir or an external dosing unit.
        const Dos1Present = 0x08;
        /// Neutralizer/rinse aid reservoir reed switch.
        ///
        /// This switch closes when neutralizer/rinse aid is present in
        /// the door-mounted reservoir.
        const DuoDosPresent = 0x10;
        /// Neutralizer reservoir switch.
        ///
        /// This switch closes when neutralizer is present in the external dosing unit.
        const Dos3Present = 0x20;
        /// Water overflow float switch.
        ///
        /// This switch closes when water is detected in the base of the machine.
        const Overflow = 0x40;
    }
}

/// Glasswasher device implementation.
///
/// Connect to a compatible glasswasher using [`Glasswasher::connect`].
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// use freemdu::device::{Device, id517::Glasswasher};
///
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut washer = Glasswasher::connect(&mut port).await?;
///
/// println!("Program type: {}", washer.query_program_type().await?);
/// println!("Drying enabled: {}", washer.query_drying_enabled().await?);
///
/// washer.start_program().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Glasswasher<P> {
    intf: Interface<P>,
    software_id: u16,
}

impl<P: Read + Write> Glasswasher<P> {
    pub(crate) async fn initialize(
        mut intf: Interface<P>,
        software_id: u16,
    ) -> Result<Self, P::Error> {
        intf.unlock_read_access(0x8542).await?;
        intf.unlock_full_access(0x6567).await?;

        // Disable ROM readout protection to access memory above 0x8000.
        // The protection is partially broken because the ROM already starts at 0x1000.
        // Its logic was likely copied from a previous device with a smaller ROM.
        intf.write_memory(0x00f4, 0x02u8).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the numerical identifier of the machine.
    ///
    /// The identifier has a length of 8 digits, e.g. `49365232`.
    /// It can be set in the service mode of the machine.
    pub async fn query_machine_number(&mut self) -> Result<u32, P::Error> {
        // The machine number is stored in reverse order, with swapped nibbles
        let num: u32 = self.intf.read_memory(0x0234).await?;
        let num = num.swap_bytes();
        let num = ((num & 0x0f0f_0f0f) << 4) | ((num & 0xf0f0_f0f0) >> 4);

        Ok(utils::decode_bcd_value(num))
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    /// It is internally stored in minutes and hours but only the hours are displayed in the service mode.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // The current time is stored as follows:
        //   - Hours: BCD values from 0x00c7 to 0x00c9
        //   - Minutes: BCD value at 0x00ca
        // When the minutes counter reaches 60, the hour value is incremented.
        let time: [u8; 4] = self.intf.read_memory(0x00c7).await?;
        let hours = utils::decode_bcd_value(u32::from_le_bytes([time[0], time[1], time[2], 0x00]));
        let mins = utils::decode_bcd_value(time[3].into());

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the status of a fault identified by its fault code.
    ///
    /// Faults may be either currently active or stored persistently in EEPROM
    /// from a previous occurrence when the machine was powered off.
    /// Returned faults do not include operating hours or occurrence count information.
    pub async fn query_fault(&mut self, code: FaultCode) -> Result<Fault, P::Error> {
        // Inlet and draining faults are not stored on the fault stack
        let ((active_addr, active_mask), stored_on_stack) = match code {
            FaultCode::TemperatureMainWash => ((0x0090, 0x01), true),
            FaultCode::TemperatureFinalRinse => ((0x0090, 0x02), true),
            FaultCode::NtcThermistorShort => ((0x0090, 0x08), true),
            FaultCode::NtcThermistorOpen => ((0x0090, 0x10), true),
            FaultCode::SpeedSensor => ((0x0090, 0x80), true),
            FaultCode::MainsFrequency => ((0x0091, 0x01), true),
            FaultCode::ProgramSelector => ((0x0091, 0x02), true),
            FaultCode::ExternalModuleI2C => ((0x0091, 0x04), true),
            FaultCode::MotorTriac => ((0x0091, 0x08), true),
            FaultCode::PressureSwitchStart => ((0x0091, 0x10), true),
            FaultCode::WaterOverflow => ((0x0091, 0x20), true),
            FaultCode::InsufficientSalt => ((0x0093, 0x01), true),
            FaultCode::PressureSwitchDraining => ((0x0095, 0x01), false),
            FaultCode::WaterInletStart => ((0x0095, 0x02), false),
            FaultCode::WaterInletPressure => ((0x0095, 0x04), false),
            FaultCode::WaterInletDuration => ((0x0095, 0x08), false),
            FaultCode::WaterInletInadvertent => ((0x0095, 0x10), false),
            FaultCode::WaterInletEnd => ((0x0095, 0x20), false),
        };

        let active: u8 = self.intf.read_memory(active_addr).await?;

        if (active & active_mask) != 0x00 {
            return Ok(Fault::Active(None));
        }

        if stored_on_stack {
            let stack: [u8; 3] = self.intf.read_memory(0x0213).await?;

            if stack.contains(&(code as u8)) {
                return Ok(Fault::Stored(None));
            }
        }

        Ok(Fault::Ok)
    }

    /// Queries the selected program.
    pub async fn query_selected_program(&mut self) -> Result<Program, P::Error> {
        Program::from_repr(self.intf.read_memory(0x0087).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program type.
    ///
    /// The program type is set according to the program selector position.
    pub async fn query_program_type(&mut self) -> Result<ProgramType, P::Error> {
        ProgramType::from_repr(self.intf.read_memory(0x0086).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries whether the drying program option is enabled.
    pub async fn query_drying_enabled(&mut self) -> Result<bool, P::Error> {
        let enabled: u8 = self.intf.read_memory(0x0078).await?;

        Ok((enabled & 0x20) != 0x00)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        // The machine sets the program phase indicator LEDs by indexing into
        // a lookup table at 0x13df, using the current program step.
        // There's no internal program phase index.
        match self.query_program_step().await? {
            0 => Ok(ProgramPhase::Idle),
            1..=10 => Ok(ProgramPhase::PreRinse),
            11..=13 => Ok(ProgramPhase::PreWash),
            14..=25 => Ok(ProgramPhase::MainWash),
            26..=31 => Ok(ProgramPhase::InterimRinse1),
            32..=36 => Ok(ProgramPhase::InterimRinse2),
            37..=47 => Ok(ProgramPhase::FinalRinse),
            48..=53 => Ok(ProgramPhase::Drying),
            54 => Ok(ProgramPhase::Finish),
            _ => Err(Error::UnexpectedMemoryValue),
        }
    }

    /// Queries the program step.
    ///
    /// The program steps range from `0` to `54`.
    /// More information about the individual steps can be found in the program chart,
    /// which is part of the technical documentation.
    pub async fn query_program_step(&mut self) -> Result<u8, P::Error> {
        // The internal step counter at 0x006e has a larger range
        // and also includes hidden steps not shown in the program chart.
        Ok(self.intf.read_memory(0x006f).await?)
    }

    /// Queries the elapsed time of the currently active program.
    pub async fn query_program_elapsed_time(&mut self) -> Result<Duration, P::Error> {
        // The elapsed time is stored BCD-encoded in minutes.
        // Depending on the display mode at 0x00cd, the display
        // shows the remaining time or current temperature in °C.
        // The raw display contents are stored from 0x00a1 to 0x00a3.
        let time: u8 = self.intf.read_memory(0x0076).await?;
        let mins = utils::decode_bcd_value(time.into());

        Ok(Duration::from_secs(u64::from(mins) * 60))
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        let actuators: u32 = self.intf.read_memory(0x00e9).await?;

        Actuator::from_bits(actuators & 0x00e0_7f80).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries which switches are currently closed.
    pub async fn query_closed_switches(&mut self) -> Result<Switch, P::Error> {
        let switches: u8 = self.intf.read_memory(0x0059).await?;

        Switch::from_bits(switches & 0x7d).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the current NTC thermistor resistance and target resistance.
    ///
    /// The resistance in `Ω` (ohms) is calculated from the ADC voltage.
    pub async fn query_ntc_resistance(&mut self) -> Result<(u32, u32), P::Error> {
        // The current resistance is converted into a BCD temperature value in °C
        // by a subroutine at 0x1b42 for diagnostic purposes.
        // However, this subroutine doesn't seem to run for some reason.
        let current: u8 = self.intf.read_memory(0x005a).await?;
        let target: u8 = self.intf.read_memory(0x00b6).await?;

        Ok((
            utils::ntc_resistance_from_adc(current),
            utils::ntc_resistance_from_adc(target),
        ))
    }

    /// Queries the current number of pulses sensed by the flow meter and the target pulse count.
    ///
    /// The flow meter produces a pulse each time a fixed volume of water enters the machine.
    /// Under normal operating conditions, one liter of water corresponds to `200` pulses.
    pub async fn query_flow_meter_pulses(&mut self) -> Result<(u16, u16), P::Error> {
        let current: u16 = self.intf.read_memory(0x0070).await?;
        let target: u16 = self.intf.read_memory(0x00bc).await?;

        Ok((current, target))
    }

    /// Starts the selected program.
    ///
    /// As the program cannot be set using the diagnostic interface,
    /// the desired program has to be selected manually using the program selector knob.
    /// This function returns an error if no program has been chosen
    /// or if a program is already running.
    pub async fn start_program(&mut self) -> Result<(), P::Error> {
        // Programs are managed by a state machine subroutine at 0x48a5.
        // The current state is stored at 0x006a. Known state values include:
        //   0x01: no program selected or running
        //   0x02: program selected and ready to start
        //   0x06: program running
        //   0x09: machine in service mode
        // Additional state values are utilized internally by the state machine.
        let state: u8 = self.intf.read_memory(0x006a).await?;

        if state == 0x02 {
            Ok(self.intf.write_memory(0x006a, 0x05u8).await?)
        } else {
            Err(Error::InvalidState)
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<P: Read + Write> Device<P> for Glasswasher<P> {
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
        DeviceKind::Glasswasher
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_MACHINE_NUMBER,
            PROP_FAULT_F1,
            PROP_FAULT_F2,
            PROP_FAULT_F4,
            PROP_FAULT_F5,
            PROP_FAULT_F8,
            PROP_FAULT_F9,
            PROP_FAULT_F10,
            PROP_FAULT_F11,
            PROP_FAULT_F23,
            PROP_FAULT_F24,
            PROP_FAULT_F25,
            PROP_FAULT_F26,
            PROP_FAULT_FA,
            PROP_FAULT_F0E,
            PROP_FAULT_F1E,
            PROP_FAULT_F2E,
            PROP_FAULT_F3E,
            PROP_FAULT_F4E,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_TYPE,
            PROP_DRYING_ENABLED,
            PROP_PROGRAM_PHASE,
            PROP_PROGRAM_STEP,
            PROP_PROGRAM_ELAPSED_TIME,
            PROP_ACTIVE_ACTUATORS,
            PROP_CLOSED_SWITCHES,
            PROP_NTC_RESISTANCE,
            PROP_FLOW_METER_PULSES,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[ACTION_START_PROGRAM]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_MACHINE_NUMBER => Ok(self.query_machine_number().await?.into()),
            // Fault
            PROP_FAULT_F1 => Ok(self
                .query_fault(FaultCode::TemperatureMainWash)
                .await?
                .into()),
            PROP_FAULT_F2 => Ok(self
                .query_fault(FaultCode::TemperatureFinalRinse)
                .await?
                .into()),
            PROP_FAULT_F4 => Ok(self.query_fault(FaultCode::NtcThermistorOpen).await?.into()),
            PROP_FAULT_F5 => Ok(self
                .query_fault(FaultCode::NtcThermistorShort)
                .await?
                .into()),
            PROP_FAULT_F8 => Ok(self.query_fault(FaultCode::SpeedSensor).await?.into()),
            PROP_FAULT_F9 => Ok(self.query_fault(FaultCode::MainsFrequency).await?.into()),
            PROP_FAULT_F10 => Ok(self.query_fault(FaultCode::ProgramSelector).await?.into()),
            PROP_FAULT_F11 => Ok(self.query_fault(FaultCode::ExternalModuleI2C).await?.into()),
            PROP_FAULT_F23 => Ok(self.query_fault(FaultCode::InsufficientSalt).await?.into()),
            PROP_FAULT_F24 => Ok(self.query_fault(FaultCode::MotorTriac).await?.into()),
            PROP_FAULT_F25 => Ok(self
                .query_fault(FaultCode::PressureSwitchStart)
                .await?
                .into()),
            PROP_FAULT_F26 => Ok(self.query_fault(FaultCode::WaterOverflow).await?.into()),
            PROP_FAULT_FA => Ok(self
                .query_fault(FaultCode::PressureSwitchDraining)
                .await?
                .into()),
            PROP_FAULT_F0E => Ok(self.query_fault(FaultCode::WaterInletStart).await?.into()),
            PROP_FAULT_F1E => Ok(self.query_fault(FaultCode::WaterInletEnd).await?.into()),
            PROP_FAULT_F2E => Ok(self
                .query_fault(FaultCode::WaterInletDuration)
                .await?
                .into()),
            PROP_FAULT_F3E => Ok(self
                .query_fault(FaultCode::WaterInletPressure)
                .await?
                .into()),
            PROP_FAULT_F4E => Ok(self
                .query_fault(FaultCode::WaterInletInadvertent)
                .await?
                .into()),
            // Operation
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_DRYING_ENABLED => Ok(self.query_drying_enabled().await?.into()),
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_PROGRAM_STEP => Ok(self.query_program_step().await?.into()),
            PROP_PROGRAM_ELAPSED_TIME => Ok(self.query_program_elapsed_time().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_CLOSED_SWITCHES => Ok(self.query_closed_switches().await?.to_string().into()),
            PROP_NTC_RESISTANCE => Ok(self.query_ntc_resistance().await?.into()),
            PROP_FLOW_METER_PULSES => Ok(self.query_flow_meter_pulses().await?.into()),
            _ => Err(Error::UnknownProperty),
        }
    }

    async fn trigger_action(
        &mut self,
        action: &Action,
        param: Option<&str>,
    ) -> Result<(), P::Error> {
        match *action {
            ACTION_START_PROGRAM => match param {
                None => self.start_program().await,
                Some(_) => Err(Error::InvalidArgument),
            },
            _ => Err(Error::UnknownAction),
        }
    }
}

impl<P> private::Sealed for Glasswasher<P> {}
