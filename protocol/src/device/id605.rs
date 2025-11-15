//! Device support for G 6xx series dishwashers.
//!
//! Supports appliances with software ID 605, which typically use an EGPL 542-C board or similar.
//!
//! A dishwasher instance can be obtained using [`Dishwasher::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, ActionKind, Device, DeviceKind, Error, Interface, Property, PropertyKind, Result,
    Value, private, utils,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::str;
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        605
    };
}
pub(super) use compatible_software_ids;

const PROP_BOARD_NUMBER: Property = Property {
    kind: PropertyKind::General,
    id: "board_number",
    name: "Board Number",
    unit: None,
};
const PROP_FAULTS: Property = Property {
    kind: PropertyKind::Failure,
    id: "faults",
    name: "Faults",
    unit: None,
};
const PROP_PROGRAM_SELECTOR: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_selector",
    name: "Program Selector",
    unit: None,
};
const PROP_PROGRAM_TYPE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_type",
    name: "Program Type",
    unit: None,
};
const PROP_TOP_SOLO_ENABLED: Property = Property {
    kind: PropertyKind::Operation,
    id: "top_solo_enabled",
    name: "Top Solo Enabled",
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
const PROP_TARGET_WATER_AMOUNT: Property = Property {
    kind: PropertyKind::Io,
    id: "target_water_amount",
    name: "Target Water Amount",
    unit: Some("ml"),
};

const ACTION_START_PROGRAM: Action = Action {
    kind: ActionKind::Operation,
    id: "start_program",
    name: "Start Program",
    params: None,
};

bitflags::bitflags! {
    /// Dishwasher fault.
    ///
    /// Each flag represents a specific fault condition that can occur in the machine.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Fault: u16 {
        /// NTC thermistor (temperature sensor) open circuit detected.
        const NtcThermistorOpen = 0x0001;
        /// NTC thermistor (temperature sensor) short circuit detected.
        const NtcThermistorShort = 0x0002;
        /// Program selection knob fault detected.
        const ProgramSelector = 0x0004;
        /// Heater fault detected.
        const Heater = 0x0008;
        /// Drainage fault detected.
        const Drainage = 0x0010;
        /// Inlet fault detected at the start of the inlet phase.
        const InletStart = 0x0020;
        /// Inlet fault detected at the end of the inlet phase.
        const InletEnd = 0x0040;
        /// Heater pressure switch fault detected during inlet phase.
        const PressureSwitchInlet = 0x0080;
        /// Heater pressure switch fault detected during heating phase.
        const PressureSwitchHeating = 0x0100;
    }
}

/// Dishwashing program type.
///
/// Represents the general category of a dishwashing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramType {
    /// No program.
    None = 0x00,
    /// Universal plus program.
    UniversalPlus = 0x03,
    /// Energy save program.
    EnergySave = 0x04,
    /// Gentle program.
    Gentle = 0x05,
    /// Universal program.
    Universal = 0x06,
    /// Economy program.
    Economy = 0x07,
    /// Pre-wash program.
    PreWash = 0x08,
    /// Intensive program.
    Intensive = 0x0a,
    /// Normal program.
    Normal = 0x0b,
    /// Test program.
    ///
    /// Only available in service mode 1.
    Test = 0x0c,
}

/// Dishwashing program phase.
///
/// Some phases may be skipped depending on the selected washing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramPhase {
    /// Program has not started yet.
    Idle,
    /// Reactivation phase.
    Reactivation,
    /// First pre-wash phase.
    PreWash1,
    /// Second pre-wash phase.
    PreWash2,
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
    /// Dishwasher actuator.
    ///
    /// Each flag represents a controllable component of the dishwasher.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u16 {
        /// Release element (drying vent) actuator.
        const ReleaseElement = 0x0001;
        /// Top solo circulation actuator.
        const TopSoloCirculation = 0x0002;
        /// Detergent dosing actuator.
        const DetergentDosing = 0x0004;
        /// Rinse aid dosing actuator.
        const RinseAidDosing = 0x0008;
        /// Reactivation actuator.
        const Reactivation = 0x0010;
        /// Inlet actuator.
        const Inlet = 0x0020;
        /// Heater actuator.
        const Heater = 0x0040;
        /// Water hardness actuator.
        const WaterHardness = 0x0080;
        /// Drying fan actuator.
        const DryingFan = 0x2000;
        /// Drain pump actuator.
        const DrainPump = 0x4000;
        /// Circulation pump actuator.
        const CirculationPump = 0x8000;
    }
}

bitflags::bitflags! {
    /// Dishwasher switch status.
    ///
    /// Each flag corresponds to a switch signal from the dishwasher's sensors.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Switch: u8 {
        /// Heater pressure switch.
        ///
        /// This switch closes when the water pressure reaches a certain threshold
        /// while the circulation pump is running, allowing the heater to be activated.
        const HeaterPressure = 0x01;
        /// Salt reservoir reed switch.
        ///
        /// This switch closes when the salt reservoir is empty.
        const SaltReservoirEmpty = 0x02;
        /// Rinse aid reservoir reed switch.
        ///
        /// This switch closes when the rinse aid reservoir is empty.
        const RinseAidReservoirEmpty = 0x04;
    }
}

/// Dishwasher device implementation.
///
/// Connect to a compatible dishwasher using [`Dishwasher::connect`].
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// use freemdu::device::{Device, id605::Dishwasher};
///
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut washer = Dishwasher::connect(&mut port).await?;
///
/// println!("Program type: {}", washer.query_program_type().await?);
/// println!("Top solo enabled: {}", washer.query_top_solo_enabled().await?);
///
/// washer.start_program().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Dishwasher<P> {
    intf: Interface<P>,
    software_id: u16,
}

impl<P: Read + Write> Dishwasher<P> {
    pub(crate) async fn initialize(
        mut intf: Interface<P>,
        software_id: u16,
    ) -> Result<Self, P::Error> {
        intf.unlock_read_access(0x1234).await?;
        intf.unlock_full_access(0x5678).await?;

        // Disable ROM readout protection to access memory above 0x8000
        intf.write_memory(0x00f4, 0x02u8).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the electronics board number of the machine.
    ///
    /// The board number consists of 8 characters, e.g. `56554705`.
    /// It can also be found on the sticker on the back side of the PCB.
    pub async fn query_board_number(&mut self) -> Result<String, P::Error> {
        let data: [u8; 8] = self.intf.read_eeprom(0x00ec).await?;
        let board = str::from_utf8(&data).map_err(|_| Error::UnexpectedMemoryValue)?;

        Ok(board.to_string())
    }

    /// Queries the stored faults.
    ///
    /// The faults are persisted in the EEPROM when turning off the machine.
    pub async fn query_faults(&mut self) -> Result<Fault, P::Error> {
        Fault::from_bits(self.intf.read_memory(0x0082).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program selection knob position.
    ///
    /// Returns the position as a numeric clock position value
    /// (e.g., `2` represents the 2 o'clock position).
    pub async fn query_program_selector(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x00af).await?)
    }

    /// Queries the program type.
    ///
    /// The program type is set according to the program selector position.
    pub async fn query_program_type(&mut self) -> Result<ProgramType, P::Error> {
        ProgramType::from_repr(self.intf.read_memory(0x0065).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries whether the top solo program option is enabled.
    pub async fn query_top_solo_enabled(&mut self) -> Result<bool, P::Error> {
        // The top solo option affects the front panel indicator lights at 0x004b,
        // which are written to port 1.
        let enabled: u8 = self.intf.read_memory(0x008e).await?;

        Ok((enabled & 0x01) != 0x00)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        // Program phases are defined in a lookup table at address 0x8c4f.
        // The phase is determined by reading the value at 0x008b to index into this table,
        // keeping only the lower nibble of the resulting value.
        // The upper nibble contains the individual program step durations.
        // This program phase is then used to set the front panel indicator lights at 0x0047,
        // by indexing into another lookup table at 0xdd54 for the LED combinations.
        ProgramPhase::from_repr(self.intf.read_memory(0x006a).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program step.
    ///
    /// The program steps range from `0` to `50`.
    /// More information about the individual steps can be found in the program chart,
    /// which is part of the technical documentation.
    pub async fn query_program_step(&mut self) -> Result<u8, P::Error> {
        // The program step is determined from a lookup table at 0xb04e,
        // which is indexed using the internal step counter at 0x008b.
        // This counter has a larger range and also includes hidden steps
        // not shown in the program chart.
        Ok(self.intf.read_memory(0x020d).await?)
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        // The active actuators at 0x009e and 0x009f are set
        // based on lookup tables at 0xe81f and 0xe89b,
        // indexed using the internal program step counter at 0x008b.
        // The actuators are then written to 0x00a0 and 0x00a1,
        // and to 0x022a and 0x022b by a subroutine at 0xc770.
        // This subroutine transforms bit 4 of 0x022b into bit 0 of 0x022a
        // before writing 0x022a to port 0.
        // The upper three bits of 0x00a1 are then used to set port 3.
        let actuators: u16 = self.intf.read_memory(0x022a).await?;

        Actuator::from_bits(actuators & 0xe0ff).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries which switches are currently closed.
    pub async fn query_closed_switches(&mut self) -> Result<Switch, P::Error> {
        Switch::from_bits(self.intf.read_memory(0x006f).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the current NTC thermistor resistance and target resistance.
    ///
    /// The resistance in `Ω` (ohms) is calculated from the ADC voltage.
    pub async fn query_ntc_resistance(&mut self) -> Result<(u32, u32), P::Error> {
        // The current resistance is converted into a BCD temperature value in °C
        // by a subroutine at 0x827c for diagnostic purposes.
        // However, this subroutine doesn't seem to run for some reason.
        let current: u8 = self.intf.read_memory(0x0061).await?;
        let target: u8 = match self.intf.read_memory(0x006c).await? {
            0xff => 0x00, // No target value set
            t => t,
        };

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
        let current: u16 = self.intf.read_memory(0x0088).await?;
        let target: u16 = self.intf.read_memory(0x00c5).await?;

        Ok((current, target))
    }

    /// Queries the target water amount.
    ///
    /// The water amount is provided in `ml` (milliliters).
    pub async fn query_target_water_amount(&mut self) -> Result<u16, P::Error> {
        // The water amount (in centiliters) is only used to
        // calculate the target pulse count in a subroutine at 0x8855.
        let amount: u16 = self.intf.read_memory(0x00d6).await?;

        Ok(amount * 10)
    }

    /// Starts the selected program.
    ///
    /// As the program cannot be set using the diagnostic interface,
    /// the desired program has to be selected manually using the program selection knob.
    /// This function returns an error if no program has been chosen
    /// or if a program is already running.
    pub async fn start_program(&mut self) -> Result<(), P::Error> {
        // Programs are managed by a state machine subroutine at 0xb5ef.
        // The current state is stored at 0x0084. Known state values include:
        //   0x01: no program selected or running
        //   0x02: program selected and ready to start
        //   0x03: machine in service mode
        //   0x06: program running
        // Additional state values are utilized internally by the state machine.
        let state: u8 = self.intf.read_memory(0x0084).await?;

        if state == 0x02 {
            Ok(self.intf.write_memory(0x0084, 0x05u8).await?)
        } else {
            Err(Error::InvalidState)
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<P: Read + Write> Device<P> for Dishwasher<P> {
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
        DeviceKind::Dishwasher
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_BOARD_NUMBER,
            PROP_FAULTS,
            PROP_PROGRAM_SELECTOR,
            PROP_PROGRAM_TYPE,
            PROP_TOP_SOLO_ENABLED,
            PROP_PROGRAM_PHASE,
            PROP_PROGRAM_STEP,
            PROP_ACTIVE_ACTUATORS,
            PROP_CLOSED_SWITCHES,
            PROP_NTC_RESISTANCE,
            PROP_FLOW_METER_PULSES,
            PROP_TARGET_WATER_AMOUNT,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[ACTION_START_PROGRAM]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_BOARD_NUMBER => Ok(self.query_board_number().await?.into()),
            // Failure
            PROP_FAULTS => Ok(self.query_faults().await?.to_string().into()),
            // Operation
            PROP_PROGRAM_SELECTOR => Ok(self.query_program_selector().await?.into()),
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_TOP_SOLO_ENABLED => Ok(self.query_top_solo_enabled().await?.into()),
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_PROGRAM_STEP => Ok(self.query_program_step().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_CLOSED_SWITCHES => Ok(self.query_closed_switches().await?.to_string().into()),
            PROP_NTC_RESISTANCE => Ok(self.query_ntc_resistance().await?.into()),
            PROP_FLOW_METER_PULSES => Ok(self.query_flow_meter_pulses().await?.into()),
            PROP_TARGET_WATER_AMOUNT => Ok(self.query_target_water_amount().await?.into()),
            _ => Err(Error::UnknownProperty),
        }
    }

    async fn trigger_action(
        &mut self,
        action: &Action,
        param: Option<Value>,
    ) -> Result<(), P::Error> {
        match *action {
            ACTION_START_PROGRAM => match param {
                None => self.start_program().await,
                _ => Err(Error::InvalidArgument),
            },
            _ => Err(Error::UnknownAction),
        }
    }
}

impl<P> private::Sealed for Dishwasher<P> {}
