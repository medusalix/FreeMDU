//! Device support for W 4xx series washing machines.
//!
//! Supports appliances with software ID 457, such as the W 471.
//! The firmware is closely related to software ID 469 (see [`id469`](crate::device::id469)),
//! but several memory addresses are shifted by one or two bytes.
//!
//! This implementation is read-only: it only unlocks read access, exposes no actions
//! and never writes to the appliance's memory. All addresses were determined by observing
//! a W 471 through the optical interface, without access to the ROM contents.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, Date, Device, DeviceKind, Error, Fault, Interface, Property, PropertyKind, Result,
    Value, private, utils,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::{str, str::FromStr, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, EnumMessage, EnumString, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        457
    };
}
pub(super) use compatible_software_ids;

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
const PROP_FAULT_F5: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f5",
    name: "F5: Detergent Overdose",
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
const PROP_PROGRAM_SPIN_SETTING: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_spin_setting",
    name: "Program Spin Setting",
    unit: None,
};
const PROP_PROGRAM_SPIN_SPEED: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_spin_speed",
    name: "Program Spin Speed",
    unit: Some("rpm"),
};
const PROP_PROGRAM_PHASE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_phase",
    name: "Program Phase",
    unit: None,
};
const PROP_LOAD_LEVEL: Property = Property {
    kind: PropertyKind::Operation,
    id: "load_level",
    name: "Load Level",
    unit: Some("kg"),
};
const PROP_PROGRAM_MAX_LOAD: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_max_load",
    name: "Program Max Load",
    unit: Some("g"),
};
const PROP_DISPLAY_CONTENTS: Property = Property {
    kind: PropertyKind::Operation,
    id: "display_contents",
    name: "Display Contents",
    unit: None,
};
const PROP_DISPLAY_INDICATOR: Property = Property {
    kind: PropertyKind::Operation,
    id: "display_indicator",
    name: "Display Indicator",
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
const PROP_DOOR_LOCKED: Property = Property {
    kind: PropertyKind::Operation,
    id: "door_locked",
    name: "Door Locked",
    unit: None,
};
const PROP_SPINNING: Property = Property {
    kind: PropertyKind::Io,
    id: "spinning",
    name: "Spinning",
    unit: None,
};
const PROP_ESTIMATED_POWER: Property = Property {
    kind: PropertyKind::Io,
    id: "estimated_power",
    name: "Estimated Power",
    unit: Some("W"),
};
const PROP_TACHOMETER_SPEED: Property = Property {
    kind: PropertyKind::Io,
    id: "tachometer_speed",
    name: "Tachometer Speed",
    unit: Some("rpm"),
};

/// What the display shows, as told by the indicator lamps beside it.
///
/// The machine weighs the dry laundry and shows the result before a program starts: the load as a
/// share of the program's maximum (_Belading %_: 25, 50, 75 or 100, nothing below 25) while the
/// door is open, then the detergent dose as a share of a full-load dose (_Wasmiddel %_: 40, 50,
/// 60, 75 or 100) once it is closed. The button above _Start_ cycles through them and the
/// remaining time.
#[derive(Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
pub enum DisplayIndicator {
    /// The display is switched off or blank.
    #[strum(message = "Off")]
    Off,
    /// The display shows the load in percent of the program's maximum, or nothing below 25 %.
    #[strum(message = "Load %")]
    Load,
    /// The display shows the detergent dose in percent of a full-load dose.
    #[strum(message = "Detergent %")]
    Detergent,
    /// The display shows the remaining program time.
    #[strum(message = "Remaining time")]
    RemainingTime,
}

impl DisplayIndicator {
    /// Decodes the indicator from the four MC14489 display bytes.
    ///
    /// The lamps for load and remaining time are driven as the fourth digit bank (1 and 2); the
    /// detergent lamp is not, so a number shown with that bank dark is the detergent dose.
    fn from_display(display: [u8; 4]) -> Self {
        let enabled = (display[3] & 0x01) != 0x00;
        let digits_blank = utils::decode_mc14489_display(display)
            .iter()
            .flatten()
            .all(|c| *c == '.');

        match display[1] >> 4 {
            _ if !enabled => Self::Off,
            0x01 => Self::Load,
            0x02 => Self::RemainingTime,
            _ if digits_blank => Self::Off,
            _ => Self::Detergent,
        }
    }
}

/// Decodes the time to the end of the program from the four MC14489 display bytes.
///
/// Only while the remaining-time lamp is lit (see [`DisplayIndicator`]): the same digits show the
/// load or the detergent dose otherwise. The machine shows minutes (`31`) or, from an hour on,
/// hours and minutes (`1.18`). The lamp's enable bit is not checked, because it drops for a moment
/// while the time is still shown.
fn remaining_time_from_display(display: [u8; 4]) -> Option<Duration> {
    if display[1] >> 4 != 0x02 {
        return None;
    }

    let text: String = utils::decode_mc14489_display(display)
        .iter()
        .flatten()
        .collect();
    let (hours, mins) = match text.trim().split_once('.') {
        Some((hours, mins)) if mins.len() == 2 => (hours.trim().parse::<u64>().ok()?, mins),
        Some(_) => return None,
        None => (0, text.trim()),
    };
    let mins = mins.parse::<u64>().ok()?;

    Some(Duration::from_secs((hours * 60 + mins) * 60))
}

/// Washing machine operating state.
///
/// Different modes can be entered by pressing specific button combinations
/// when turning on the machine.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum OperatingState {
    /// The door is open. Washing programs cannot be started.
    #[strum(message = "Door open")]
    DoorOpen,
    /// Default mode when the machine is turned on.
    #[strum(message = "Ready")]
    ProgramIdle,
    /// A washing program is currently running.
    #[strum(message = "Running")]
    ProgramRunning,
    /// The washing program has finished.
    #[strum(message = "Finished")]
    ProgramFinished,
    /// Service programming mode, providing access to all machine and program options.
    ///
    /// Entered by holding the _Delay start h/min_ and _Water plus_ buttons
    /// when turning on the machine.
    #[strum(message = "Service programming")]
    ServiceProgramming,
    /// Customer programming mode, with a limited subset of the service programming options.
    ///
    /// Entered by holding the _Intensive_/_Short_ and _Water plus_ buttons when turning on the machine.
    #[strum(message = "Customer programming")]
    CustomerProgramming,
    /// Service mode. Allows viewing stored faults and testing actuators.
    ///
    /// Entered by holding the _Delay start h/min_ and _Intensive_/_Short_ buttons
    /// when turning on the machine.
    #[strum(message = "Service mode")]
    Service,
}

/// Washing machine program.
///
/// Each variant represents a position of the machine's program selector knob.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Program {
    /// Finish position (no program selected).
    #[strum(message = "Finish")]
    Finish,
    /// Cottons program, 95 °C.
    #[strum(message = "Cottons 95 °C")]
    Cottons95,
    /// Cottons program, 75 °C.
    #[strum(message = "Cottons 75 °C")]
    Cottons75,
    /// Cottons program, 60 °C.
    #[strum(message = "Cottons 60 °C")]
    Cottons60,
    /// Cottons program, 40 °C.
    #[strum(message = "Cottons 40 °C")]
    Cottons40,
    /// Cottons program, 30 °C.
    #[strum(message = "Cottons 30 °C")]
    Cottons30,
    /// Minimum iron program, 60 °C.
    #[strum(message = "Minimum iron 60 °C")]
    MinimumIron60,
    /// Minimum iron program, 50 °C.
    #[strum(message = "Minimum iron 50 °C")]
    MinimumIron50,
    /// Minimum iron program, 40 °C.
    #[strum(message = "Minimum iron 40 °C")]
    MinimumIron40,
    /// Minimum iron program, 30 °C.
    #[strum(message = "Minimum iron 30 °C")]
    MinimumIron30,
    /// Drain/spin program.
    #[strum(message = "Drain/spin")]
    DrainSpin,
    /// Separate rinse program.
    #[strum(message = "Separate rinse")]
    SeparateRinse,
    /// Starch program.
    #[strum(message = "Starch")]
    Starch,
    /// Mixed wash program, 40 °C.
    #[strum(message = "Mixed wash 40 °C")]
    MixedWash40,
    /// Quick wash program, 40 °C.
    #[strum(message = "Quick wash 40 °C")]
    QuickWash40,
    /// Woolens program, cold.
    #[strum(message = "Woolens cold")]
    WoolensCold,
    /// Woolens program, 30 °C.
    #[strum(message = "Woolens 30 °C")]
    Woolens30,
    /// Woolens program, 40 °C.
    #[strum(message = "Woolens 40 °C")]
    Woolens40,
    /// Silks program, 30 °C.
    #[strum(message = "Silks 30 °C")]
    Silks30,
    /// Delicates program, cold.
    #[strum(message = "Delicates cold")]
    DelicatesCold,
    /// Delicates program, 30 °C.
    #[strum(message = "Delicates 30 °C")]
    Delicates30,
    /// Delicates program, 40 °C.
    #[strum(message = "Delicates 40 °C")]
    Delicates40,
}

impl Program {
    /// The most dry laundry the program takes, in grams, from the W 471 manual (M.-Nr. 05 645 660).
    ///
    /// The machine shows its weighing as a share of this, in quarters, before a program starts.
    #[must_use]
    pub fn max_load(self) -> Option<u32> {
        match self {
            Self::Finish => None,
            Self::Cottons95
            | Self::Cottons75
            | Self::Cottons60
            | Self::Cottons40
            | Self::Cottons30
            | Self::DrainSpin
            | Self::SeparateRinse
            | Self::Starch => Some(5000),
            Self::MixedWash40 => Some(3000),
            Self::MinimumIron60
            | Self::MinimumIron50
            | Self::MinimumIron40
            | Self::MinimumIron30
            | Self::QuickWash40 => Some(2500),
            Self::WoolensCold | Self::Woolens30 | Self::Woolens40 => Some(2000),
            Self::Silks30 | Self::DelicatesCold | Self::Delicates30 | Self::Delicates40 => {
                Some(1000)
            }
        }
    }
}

/// Washing program type.
///
/// Represents the general category of a washing program.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramType {
    /// No program.
    #[strum(message = "None")]
    None = 0x00,
    /// Cottons program.
    #[strum(message = "Cottons")]
    Cottons = 0x01,
    /// Minimum iron program.
    #[strum(message = "Minimum iron")]
    MinimumIron = 0x02,
    /// Delicates program.
    #[strum(message = "Delicates")]
    Delicates = 0x03,
    /// Woolens program.
    #[strum(message = "Woolens")]
    Woolens = 0x04,
    /// Quick wash program.
    #[strum(message = "Quick wash")]
    QuickWash = 0x05,
    /// Starch program.
    #[strum(message = "Starch")]
    Starch = 0x06,
    /// Drain/spin program.
    #[strum(message = "Drain/spin")]
    DrainSpin = 0x07,
    /// Separate rinse program.
    #[strum(message = "Separate rinse")]
    SeparateRinse = 0x09,
    /// Mixed wash program.
    #[strum(message = "Mixed wash")]
    MixedWash = 0x0a,
    /// Silks program.
    #[strum(message = "Silks")]
    Silks = 0x0b,
}

bitflags::bitflags! {
    /// Washing program option.
    ///
    /// Each flag represents an optional feature that can be enabled for a program.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct ProgramOption: u8 {
        /// Soak option enabled.
        const Soak = 0x10;
        /// Pre-wash option enabled.
        const PreWash = 0x20;
        /// Water plus option enabled.
        const WaterPlus = 0x40;
        /// Intensive or short option enabled.
        ///
        /// The actual effect depends on the machine's programming configuration.
        const IntensiveShort = 0x80;
    }
}

/// Washing program spin setting.
///
/// The actual spin speed depends on the machine's programming configuration.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum SpinSetting {
    /// No spin.
    #[strum(message = "No spin")]
    WithoutSpin,
    /// Rinse hold (spin is paused to prevent creasing).
    #[strum(message = "Rinse hold")]
    RinseHold,
    /// Minimum spin speed.
    #[strum(message = "Minimum")]
    SpinMin,
    /// Low spin speed.
    #[strum(message = "Low")]
    SpinLow,
    /// Medium spin speed.
    #[strum(message = "Medium")]
    SpinMed,
    /// High spin speed.
    #[strum(message = "High")]
    SpinHigh,
    /// Very high spin speed.
    #[strum(message = "Very high")]
    SpinVeryHigh,
    /// Maximum spin speed.
    #[strum(message = "Maximum")]
    SpinMax,
}

/// Washing program phase.
///
/// Phases may not always execute in the defined order and some phases
/// may be skipped depending on the selected washing program.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramPhase {
    /// Program has not started yet.
    #[strum(message = "Not started")]
    Idle,
    /// Program start scheduled with delay start function.
    #[strum(message = "Delayed start")]
    DelayedStart,
    /// First soak/pre-wash phase.
    #[strum(message = "Soak/pre-wash 1")]
    SoakPreWash1,
    /// Second soak/pre-wash phase.
    #[strum(message = "Soak/pre-wash 2")]
    SoakPreWash2,
    /// Main wash phase.
    #[strum(message = "Main wash")]
    MainWash,
    /// First rinse phase.
    #[strum(message = "Rinse 1")]
    Rinse1,
    /// Second rinse phase.
    #[strum(message = "Rinse 2")]
    Rinse2,
    /// Third rinse phase.
    #[strum(message = "Rinse 3")]
    Rinse3,
    /// Fourth rinse phase.
    #[strum(message = "Rinse 4")]
    Rinse4,
    /// Fifth rinse phase.
    #[strum(message = "Rinse 5")]
    Rinse5,
    /// Rinse hold phase.
    #[strum(message = "Rinse hold")]
    RinseHold,
    /// Drain phase.
    #[strum(message = "Drain")]
    Drain,
    /// Final spin phase.
    #[strum(message = "Final spin")]
    FinalSpin,
    /// Anti-crease/finish phase.
    #[strum(message = "Anti-crease/finish")]
    AntiCreaseFinish,
}

bitflags::bitflags! {
    /// Washing machine actuator.
    ///
    /// Each flag represents a controllable component of the washing machine.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u16 {
        /// Drain pump actuator.
        const DrainPump = 0x0004;
        /// Heater actuator.
        const Heater = 0x0020;
        /// Water diverter motor actuator.
        const WaterDiverter = 0x0080;
        /// Warm water valve actuator.
        const WarmWater = 0x0200;
        /// Cold water valve actuator.
        const ColdWater = 0x0400;
    }
}

/// Power draw of a W 471, measured with the house's smart meter (P1, a reading every 10 seconds)
/// over a Cottons 60 run while the rest of the house drew a steady 150 W or so.
mod power {
    /// The heater switching showed as steps of 2030 to 2110 W; the replacement element
    /// (Miele 11905530) is rated 2.1 kW at 230 V.
    pub const HEATER: u32 = 2050;
    /// Above the base load while washing with the heater off and the drum slow: median 168 W.
    pub const RUNNING: u32 = 170;
    /// Added per 1000 rpm of drum speed in spins (a fit gave 174 W plus 234 W per 1000 rpm).
    pub const DRUM_PER_1000_RPM: u32 = 230;
    /// Switched on with no program running; from a W 925 of the same era.
    pub const STANDBY: u32 = 8;
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
        // Read access is sufficient for all properties below.
        // Full access is deliberately not unlocked: the address of the ROM readout protection
        // flag is unknown for this software ID (0x02e9, as used by software ID 469,
        // is part of an EEPROM shadow area here and must not be written).
        intf.unlock_read_access(0x43ea).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the manufacturing/inspection date of the machine.
    pub async fn query_manufacturing_date(&mut self) -> Result<Date, P::Error> {
        let date: [u8; 4] = self.intf.read_eeprom(0x01ce).await?;

        Ok(Date::new(
            u16::from(date[0]) + u16::from(date[1]) * 100,
            date[2],
            date[3],
        ))
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // The current time is stored as follows:
        //   - Minutes: binary value at 0x0052
        //   - Hours: BCD values from 0x0053 to 0x0055
        let time: [u8; 4] = self.intf.read_memory(0x0052).await?;
        let mins = time[0];
        let hours = utils::decode_bcd_value(u32::from_le_bytes([time[1], time[2], time[3], 0x00]));

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the operating state.
    pub async fn query_operating_state(&mut self) -> Result<OperatingState, P::Error> {
        OperatingState::from_repr(self.intf.read_memory(0x00cd).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the selected program.
    pub async fn query_selected_program(&mut self) -> Result<Program, P::Error> {
        Program::from_repr(self.intf.read_memory(0x00b5).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program type.
    ///
    /// The program type is set according to the program selector position.
    pub async fn query_program_type(&mut self) -> Result<ProgramType, P::Error> {
        ProgramType::from_repr(self.intf.read_memory(0x00dd).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program temperature.
    ///
    /// The program temperature is set according to the program
    /// selector position and provided in `°C` (degrees Celsius).
    /// Some programs use a slightly lower temperature than selected.
    pub async fn query_program_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x00de).await?)
    }

    /// Queries the program options.
    ///
    /// The program options are set using the buttons on the front panel of the machine.
    pub async fn query_program_options(&mut self) -> Result<ProgramOption, P::Error> {
        ProgramOption::from_bits(self.intf.read_memory(0x0058).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program spin setting.
    pub async fn query_program_spin_setting(&mut self) -> Result<SpinSetting, P::Error> {
        SpinSetting::from_repr(self.intf.read_memory(0x0057).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program spin speed.
    ///
    /// The spin speed is provided in `rpm` (revolutions per minute)
    /// and may not correspond exactly to the labels on the front panel.
    pub async fn query_program_spin_speed(&mut self) -> Result<u16, P::Error> {
        let speed: u8 = self.intf.read_memory(0x00e9).await?;

        Ok(u16::from(speed) * 50)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        ProgramPhase::from_repr(self.intf.read_memory(0x00a2).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the contents of the seven-segment display.
    ///
    /// The machine typically displays the remaining time of the selected program.
    pub async fn query_display_contents(&mut self) -> Result<String, P::Error> {
        let display: [u8; 4] = self.intf.read_memory(0x009e).await?;

        Ok(utils::decode_mc14489_display(display)
            .iter()
            .flatten()
            .collect())
    }

    /// Queries which reading the display shows (see [`DisplayIndicator`]).
    ///
    /// Together with [`Self::query_display_contents`] this gives the load and detergent dose the
    /// machine has measured; they are on the display only, so a caller that wants to keep them
    /// has to remember them while their lamp is lit.
    pub async fn query_display_indicator(&mut self) -> Result<DisplayIndicator, P::Error> {
        let display: [u8; 4] = self.intf.read_memory(0x009e).await?;

        Ok(DisplayIndicator::from_display(display))
    }

    /// Queries the time left until the end of the program, as shown on the display.
    ///
    /// Fails with [`Error::InvalidState`] while the display shows something else, such as the
    /// load or detergent dose before a start, or nothing, and during a delayed start, when the
    /// display may count down to the start instead.
    pub async fn query_remaining_time(&mut self) -> Result<Duration, P::Error> {
        if self.query_program_phase().await? == ProgramPhase::DelayedStart {
            return Err(Error::InvalidState);
        }

        let display: [u8; 4] = self.intf.read_memory(0x009e).await?;

        remaining_time_from_display(display).ok_or(Error::InvalidState)
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        let actuators: u16 = self.intf.read_memory(0x007d).await?;

        // Only the drain pump, heater and cold water valve bits have been observed so far.
        // Unknown bits are ignored instead of being treated as an error.
        Ok(Actuator::from_bits_truncate(actuators))
    }

    /// Queries the laundry load level, the machine's own estimate of the dry load.
    ///
    /// At the same address as on software ID 469. The machine weighs the dry laundry when the door
    /// is closed after loading and keeps the result for the whole program, so it can be read at
    /// any time. It reads 5 before anything is weighed, while the door is open: the most the
    /// machine takes. About ten minutes into the main wash the machine may revise it from how
    /// much water the laundry took up, and extend the remaining time to match: a Cottons 40 load
    /// went from 1 to 3 and its time from 41 minutes to 1:08.
    ///
    /// The value appears to be whole kilograms, rounded down. Every loading on a W 471 fits
    /// that against what its display showed (the load as a quarter of the program's maximum, see
    /// [`Program::max_load`]): 3 and 4 for two loads both shown as 75 % of 5 kg, 1 for one shown
    /// as 25 % and for two below 25 %. Not yet confirmed with weighed laundry.
    pub async fn query_load_level(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x004a).await?)
    }

    /// Queries the most dry laundry the selected program takes, in grams (see
    /// [`Program::max_load`]); 0 at the _Finish_ position.
    pub async fn query_program_max_load(&mut self) -> Result<u32, P::Error> {
        Ok(self.query_selected_program().await?.max_load().unwrap_or(0))
    }

    /// Queries the detergent overdose fault (F5), lit on the panel as too much foam.
    ///
    /// The status bit is the one software ID 469 uses. It was seen set from the start of the
    /// second rinse until the door was opened, in both Cottons runs of a day on which the panel
    /// showed the warning. Only the active state is reported: where this firmware stores past
    /// faults is not known (0x004e, which holds them on 469, read `0xf8` throughout).
    pub async fn query_detergent_overdose(&mut self) -> Result<Fault, P::Error> {
        let state: u8 = self.intf.read_memory(0x0044).await?;

        Ok(if (state & 0x08) != 0x00 {
            Fault::Active(None)
        } else {
            Fault::Ok
        })
    }

    /// Queries the current temperature sensed by the NTC thermistor and the target temperature.
    ///
    /// The temperature is provided in `°C` (degrees Celsius).
    pub async fn query_temperature(&mut self) -> Result<(u8, u8), P::Error> {
        let [current, target] = self.intf.read_memory(0x0136).await?;

        Ok((current, target))
    }

    /// Queries the current water level sensed by the analog pressure sensor and the target level.
    ///
    /// The water level is provided in `mmH₂O` (millimeters of water).
    pub async fn query_water_level(&mut self) -> Result<(u8, u8), P::Error> {
        let [current, target] = self.intf.read_memory(0x007f).await?;

        Ok((current, target))
    }

    /// Queries whether the door is locked.
    ///
    /// The lock closes when a program starts. After the program it opens only when the door
    /// button is pressed, and the button works once the drum has stopped (see
    /// [`Self::query_spinning`]): on a Cottons 60 run the lock stayed closed through 31 minutes of
    /// anti-crease until the button was pressed. So this tells when the door was opened, not
    /// when it could be.
    pub async fn query_door_locked(&mut self) -> Result<bool, P::Error> {
        let state: u8 = self.intf.read_memory(0x0044).await?;

        Ok((state & 0x02) != 0x00)
    }

    /// Queries whether the drum is spinning: from the start of each spin until the drum has run
    /// down after it.
    ///
    /// The motor control byte reads `0x0c` while a spin is driven, `0x05` while the drum runs
    /// down and `0x00` otherwise, including the slow tumbling of washing and anti-crease. The
    /// tachometer reads 0 during the run-down, so this is what tells that the drum has stopped
    /// after the final spin and the door button will open the door: 50 seconds after
    /// [`OperatingState::ProgramFinished`] following a 1600 rpm spin, 10 seconds after one held
    /// at 670 rpm.
    pub async fn query_spinning(&mut self) -> Result<bool, P::Error> {
        let state: u8 = self.intf.read_memory(0x0099).await?;

        Ok(state != 0x00)
    }

    /// Queries an estimate of the machine's power draw in `W` (watts), from what it is doing.
    ///
    /// The heater, a flat figure while a program runs (motor, pumps, valves, electronics) or
    /// standby otherwise, and an amount that grows with the drum speed. Measured on one W 471
    /// (see the constants); the whole Cottons 60 run came to 0.75 kWh this way against 0.73 to
    /// 0.76 kWh on the meter. Integrating it over time gives the energy.
    pub async fn query_estimated_power(&mut self) -> Result<u32, P::Error> {
        let running = self.query_operating_state().await? == OperatingState::ProgramRunning;
        let heating = self
            .query_active_actuators()
            .await?
            .contains(Actuator::Heater);
        let (rpm, _) = self.query_tachometer_speed().await?;

        let base = if running {
            power::RUNNING
        } else {
            power::STANDBY
        };
        let heater = if heating { power::HEATER } else { 0 };

        Ok(base + heater + power::DRUM_PER_1000_RPM * u32::from(rpm) / 1000)
    }

    /// Queries the drum speed measured by the tachometer generator and the target speed.
    ///
    /// Both are provided in `rpm` (revolutions per minute). The target is set during the final
    /// spin only; the measured speed also shows the short spins between rinses. A large gap
    /// between the two during the final spin means the machine is waiting for the load to
    /// balance. When the program ends, both read 0 while the drum is still running down.
    pub async fn query_tachometer_speed(&mut self) -> Result<(u16, u16), P::Error> {
        let current: u16 = self.intf.read_memory(0x01a3).await?;
        let target: u16 = self.intf.read_memory(0x028a).await?;

        Ok((current, target))
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

    fn value_name(&self, prop: &Property, val: &Value) -> Option<&'static str> {
        let Value::String(code) = val else {
            return None;
        };

        match *prop {
            PROP_OPERATING_STATE => OperatingState::from_str(code).ok()?.get_message(),
            PROP_SELECTED_PROGRAM => Program::from_str(code).ok()?.get_message(),
            PROP_PROGRAM_TYPE => ProgramType::from_str(code).ok()?.get_message(),
            PROP_PROGRAM_SPIN_SETTING => SpinSetting::from_str(code).ok()?.get_message(),
            PROP_PROGRAM_PHASE => ProgramPhase::from_str(code).ok()?.get_message(),
            PROP_DISPLAY_INDICATOR => DisplayIndicator::from_str(code).ok()?.get_message(),
            _ => None,
        }
    }

    fn software_id(&self) -> u16 {
        self.software_id
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::WashingMachine
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_MANUFACTURING_DATE,
            PROP_OPERATING_TIME,
            PROP_FAULT_F5,
            PROP_OPERATING_STATE,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_TYPE,
            PROP_PROGRAM_TEMPERATURE,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_SPIN_SETTING,
            PROP_PROGRAM_SPIN_SPEED,
            PROP_PROGRAM_PHASE,
            PROP_LOAD_LEVEL,
            PROP_PROGRAM_MAX_LOAD,
            PROP_DISPLAY_CONTENTS,
            PROP_DISPLAY_INDICATOR,
            PROP_REMAINING_TIME,
            PROP_ACTIVE_ACTUATORS,
            PROP_DOOR_LOCKED,
            PROP_SPINNING,
            PROP_TEMPERATURE,
            PROP_WATER_LEVEL,
            PROP_TACHOMETER_SPEED,
            PROP_ESTIMATED_POWER,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_MANUFACTURING_DATE => Ok(self.query_manufacturing_date().await?.into()),
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            // Faults
            PROP_FAULT_F5 => Ok(self.query_detergent_overdose().await?.into()),
            // Operation
            PROP_OPERATING_STATE => Ok(self.query_operating_state().await?.to_string().into()),
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_PROGRAM_TEMPERATURE => Ok(self.query_program_temperature().await?.into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_SPIN_SETTING => {
                Ok(self.query_program_spin_setting().await?.to_string().into())
            }
            PROP_PROGRAM_SPIN_SPEED => Ok(self.query_program_spin_speed().await?.into()),
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_LOAD_LEVEL => Ok(self.query_load_level().await?.into()),
            PROP_PROGRAM_MAX_LOAD => Ok(self.query_program_max_load().await?.into()),
            PROP_DISPLAY_CONTENTS => Ok(self.query_display_contents().await?.into()),
            PROP_DISPLAY_INDICATOR => Ok(self.query_display_indicator().await?.to_string().into()),
            PROP_REMAINING_TIME => Ok(self.query_remaining_time().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_TEMPERATURE => Ok(self.query_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
            PROP_DOOR_LOCKED => Ok(self.query_door_locked().await?.into()),
            PROP_SPINNING => Ok(self.query_spinning().await?.into()),
            PROP_TACHOMETER_SPEED => Ok(self.query_tachometer_speed().await?.into()),
            PROP_ESTIMATED_POWER => Ok(self.query_estimated_power().await?.into()),
            _ => Err(Error::UnknownProperty),
        }
    }

    async fn trigger_action(
        &mut self,
        _action: &Action,
        _param: Option<&str>,
    ) -> Result<(), P::Error> {
        Err(Error::UnknownAction)
    }
}

impl<P> private::Sealed for WashingMachine<P> {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::vec_deque::VecDeque, string::ToString};

    #[test]
    fn value_names() {
        let dev = WashingMachine {
            intf: Interface::new(VecDeque::<u8>::new()),
            software_id: 457,
        };
        let name = |prop: &Property, code: &str| dev.value_name(prop, &code.to_string().into());

        assert_eq!(
            name(&PROP_OPERATING_STATE, "ProgramRunning"),
            Some("Running")
        );
        assert_eq!(
            name(&PROP_SELECTED_PROGRAM, "QuickWash40"),
            Some("Quick wash 40 °C")
        );
        assert_eq!(
            name(&PROP_PROGRAM_TYPE, "MinimumIron"),
            Some("Minimum iron")
        );
        assert_eq!(name(&PROP_PROGRAM_SPIN_SETTING, "SpinMax"), Some("Maximum"));
        assert_eq!(
            name(&PROP_PROGRAM_PHASE, "AntiCreaseFinish"),
            Some("Anti-crease/finish")
        );
        assert_eq!(
            name(&PROP_DISPLAY_INDICATOR, "Detergent"),
            Some("Detergent %")
        );
        assert_eq!(name(&PROP_PROGRAM_PHASE, "NoSuchPhase"), None);
        assert_eq!(
            dev.value_name(&PROP_TEMPERATURE, &Value::Sensor(19, 0)),
            None
        );
    }

    #[test]
    fn max_load() {
        assert_eq!(Program::Finish.max_load(), None);
        assert_eq!(Program::Cottons40.max_load(), Some(5000));
        assert_eq!(Program::QuickWash40.max_load(), Some(2500));
        assert_eq!(Program::Woolens30.max_load(), Some(2000));

        for repr in 1..=u8::MAX {
            if let Some(program) = Program::from_repr(repr) {
                assert!(
                    program.max_load().is_some(),
                    "{program} has no maximum load"
                );
            }
        }
    }

    #[test]
    fn display_indicator() {
        // Display bytes from a W 471, 2026-10-17
        let cases = [
            ([0x70, 0x15, 0x80, 0x73], DisplayIndicator::Load), // "75", door open
            ([0x00, 0x10, 0x91, 0x7f], DisplayIndicator::Load), // blank, below 25 %
            ([0x70, 0x05, 0x80, 0x73], DisplayIndicator::Detergent), // "75", door closed
            ([0x40, 0x00, 0x80, 0x73], DisplayIndicator::Detergent), // "40"
            ([0x11, 0x28, 0x90, 0x71], DisplayIndicator::RemainingTime), // "1.18"
            ([0x00, 0x20, 0x80, 0x77], DisplayIndicator::RemainingTime), // "0", finished
            ([0x00, 0x00, 0x80, 0x7e], DisplayIndicator::Off),  // switched off
            ([0x00, 0x00, 0x90, 0x7f], DisplayIndicator::Off),  // blank
        ];

        for (display, indicator) in cases {
            assert_eq!(
                DisplayIndicator::from_display(display),
                indicator,
                "{display:02x?}"
            );
        }
    }

    #[test]
    fn remaining_time() {
        // Display bytes from a W 471's memory log, 2026-10-13 to 2026-10-20
        let mins = |m: u64| Some(Duration::from_secs(m * 60));
        let cases = [
            ([0x30, 0x21, 0x81, 0x73], mins(31)), // "31"
            ([0x01, 0x25, 0x90, 0x71], mins(65)), // "1.05"
            ([0x11, 0x28, 0x90, 0x71], mins(78)), // "1.18"
            ([0x30, 0x29, 0x80, 0x72], mins(39)), // "39", enable bit down for a moment
            ([0x00, 0x20, 0x80, 0x77], mins(0)),  // "0", finished
            ([0x00, 0x20, 0x80, 0x71], mins(0)),  // "000"
            ([0x40, 0x00, 0x80, 0x73], None),     // "40", the detergent dose before a start
            ([0x70, 0x15, 0x80, 0x73], None),     // "75", the load
            ([0x00, 0x20, 0x80, 0x7e], None),     // switched off
            ([0x00, 0x00, 0x90, 0x7f], None),     // blank
        ];

        for (display, time) in cases {
            assert_eq!(remaining_time_from_display(display), time, "{display:02x?}");
        }
    }

    #[test]
    fn every_variant_has_a_name() {
        for repr in 0..=u8::MAX {
            let names = [
                OperatingState::from_repr(repr).map(|v| v.get_message()),
                Program::from_repr(repr).map(|v| v.get_message()),
                ProgramType::from_repr(repr).map(|v| v.get_message()),
                SpinSetting::from_repr(repr).map(|v| v.get_message()),
                ProgramPhase::from_repr(repr).map(|v| v.get_message()),
            ];

            for name in names.into_iter().flatten() {
                assert!(name.is_some(), "value {repr:#04x} has no name");
            }
        }
    }
}
