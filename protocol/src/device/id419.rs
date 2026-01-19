//! Device support for W 8xx/9xx series washing machines.
//!
//! Supports appliances with software ID 419, which typically use an EDPW 206 board or similar.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, ActionKind, ActionParameters, Device, DeviceKind, Error, Interface, Property,
    PropertyKind, Result, Value, private, utils,
};
use alloc::{boxed::Box, string::ToString};
use bitflags_derive::{FlagsDebug, FlagsDisplay, FlagsFromStr};
use core::{str, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, EnumString, FromRepr, VariantNames};

macro_rules! compatible_software_ids {
    () => {
        419
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
const PROP_ACTIVE_ACTUATORS: Property = Property {
    kind: PropertyKind::Io,
    id: "active_actuators",
    name: "Active Actuators",
    unit: None,
};
const PROP_NTC_RESISTANCE: Property = Property {
    kind: PropertyKind::Io,
    id: "ntc_resistance",
    name: "NTC Resistance",
    unit: Some("Ω"),
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

const ACTION_SET_PROGRAM_OPTIONS: Action = Action {
    kind: ActionKind::Operation,
    id: "set_program_options",
    name: "Set Program Options",
    params: Some(ActionParameters::Flags(&[
        "Soak",
        "PreWash",
        "WaterPlus",
        "Short",
    ])),
};
const ACTION_SET_PROGRAM_SPIN_SETTING: Action = Action {
    kind: ActionKind::Operation,
    id: "set_program_spin_setting",
    name: "Set Program Spin Setting",
    params: Some(ActionParameters::Enumeration(SpinSetting::VARIANTS)),
};
const ACTION_START_PROGRAM: Action = Action {
    kind: ActionKind::Operation,
    id: "start_program",
    name: "Start Program",
    params: None,
};

bitflags::bitflags! {
    /// Washing machine fault.
    ///
    /// Each flag represents a specific fault condition that can occur in the machine.
    /// Multiple faults may be active simultaneously.
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
/// Different modes can be entered by pressing specific button combinations
/// when turning on the machine.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum OperatingMode {
    /// Default mode when the machine is turned on.
    ProgramIdle = 0x01,
    /// A washing program is currently running.
    ProgramRunning = 0x02,
    /// The washing program has finished.
    ProgramFinished = 0x03,
    /// Service programming mode, providing access to all machine and program options.
    ///
    /// Entered by holding the _Water plus_ and _Start_ buttons when turning on the machine.
    ServiceProgramming = 0x04,
    /// Service mode. Allows viewing stored faults and testing actuators.
    ///
    /// Entered by holding the _Short_ and _Start_ buttons when turning on the machine.
    Service = 0x05,
    /// Customer programming mode, with a limited subset of the service programming options.
    ///
    /// Entered by holding the _Short_ and _Water plus_ buttons when turning on the machine.
    CustomerProgramming = 0x06,
}

/// Washing program selection knob position.
///
/// Each variant represents a position of the machine's program selection knob.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum SelectorPosition {
    /// Finish position.
    Finish,
    /// Cottons program, 95 °C.
    Cottons95,
    /// Cottons program, 75 °C.
    Cottons75,
    /// Cottons program, 60 °C.
    Cottons60,
    /// Cottons program, 40 °C.
    Cottons40,
    /// Cottons program, 30 °C.
    Cottons30,
    /// Minimum iron program, 60 °C.
    MinimumIron60,
    /// Minimum iron program, 50 °C.
    MinimumIron50,
    /// Minimum iron program, 40 °C.
    MinimumIron40,
    /// Minimum iron program, 30 °C.
    MinimumIron30,
    /// Delicates program, 40 °C.
    Delicates40,
    /// Delicates program, 30 °C.
    Delicates30,
    /// Delicates program, cold.
    DelicatesCold,
    /// Woolens program, 40 °C.
    Woolens40,
    /// Woolens program, 30 °C.
    Woolens30,
    /// Woolens program, cold.
    WoolensCold,
    /// Quick wash program, 40 °C.
    QuickWash40,
    /// Starch program.
    Starch,
    /// Spin program.
    Spin,
    /// Drain program.
    Drain,
    /// Separate rinse program.
    SeparateRinse,
    /// Mixed wash program, 40 °C.
    MixedWash40,
}

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
    MinimumIron = 0x02,
    /// Delicates program.
    Delicates = 0x03,
    /// Woolens program.
    Woolens = 0x04,
    /// Quick wash program.
    QuickWash = 0x05,
    /// Starch program.
    Starch = 0x06,
    /// Spin program.
    Spin = 0x07,
    /// Drain program.
    Drain = 0x08,
    /// Separate rinse program.
    SeparateRinse = 0x09,
    /// Mixed wash program.
    MixedWash = 0x0a,
}

bitflags::bitflags! {
    /// Washing program option.
    ///
    /// Each flag represents an optional feature that can be enabled for a program.
    #[derive(FlagsDisplay, FlagsFromStr, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct ProgramOption: u8 {
        /// Soak option enabled.
        const Soak = 0x10;
        /// Pre-wash option enabled.
        const PreWash = 0x20;
        /// Water plus option enabled.
        const WaterPlus = 0x40;
        /// Short option enabled.
        const Short = 0x80;
    }
}

/// Washing program spin setting.
///
/// The actual spin speed depends on the machine's programming configuration.
#[derive(FromRepr, Display, EnumString, VariantNames, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum SpinSetting {
    /// No spin.
    WithoutSpin,
    /// Rinse hold (spin is paused to prevent creasing).
    RinseHold,
    /// Minimum spin speed.
    SpinMin,
    /// Low spin speed.
    SpinLow,
    /// Medium spin speed.
    SpinMed,
    /// High spin speed.
    SpinHigh,
    /// Very high spin speed.
    SpinVeryHigh,
    /// Maximum spin speed.
    SpinMax,
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
    /// Program start scheduled with delay start function.
    DelayedStart,
    /// First soak/pre-wash phase.
    SoakPreWash1,
    /// Second soak/pre-wash phase.
    SoakPreWash2,
    /// Main wash phase.
    MainWash,
    /// First rinse phase.
    Rinse1,
    /// Second rinse phase.
    Rinse2,
    /// Third rinse phase.
    Rinse3,
    /// Fourth rinse phase.
    Rinse4,
    /// Fifth rinse phase.
    Rinse5,
    /// Rinse hold phase.
    RinseHold,
    /// Drain phase.
    Drain,
    /// Final spin phase.
    FinalSpin,
    /// Anti-crease/finish phase.
    AntiCreaseFinish,
}

bitflags::bitflags! {
    /// Washing machine actuator.
    ///
    /// Each flag represents a controllable component of the washing machine.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u16 {
        /// Motor field switch relay actuator.
        const FieldSwitch = 0x0001;
        /// Drain pump actuator.
        const DrainPump = 0x0002;
        /// Reverse relay actuator.
        const Reverse = 0x0010;
        /// Heater actuator.
        const Heater = 0x0020;
        /// Softener compartment actuator.
        const Softener = 0x0040;
        /// Pre-wash compartment actuator.
        const PreWash = 0x0080;
        /// Main wash compartment actuator.
        const MainWash = 0x2000;
        /// Warm water actuator.
        const WarmWater = 0x4000;
    }
}

/// Washing machine device implementation.
///
/// Connect to a compatible washing machine using [`WashingMachine::connect`].
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// use freemdu::device::{Device, id419::WashingMachine};
///
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut machine = WashingMachine::connect(&mut port).await?;
///
/// println!("Program type: {}", machine.query_program_type().await?);
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
        // Legacy protocol requires dummy bytes
        intf.enable_dummy_bytes().await?;

        intf.unlock_read_access(0xb4ee).await?;
        intf.unlock_full_access(0x4e83).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the ROM code of the machine's microcontroller.
    ///
    /// The ROM code is typically a small number, e.g. `4`.
    pub async fn query_rom_code(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0xffdf).await?)
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    /// It is internally stored in minutes and hours but only the hours are displayed in the service mode.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // The current time is stored as follows:
        //   - Minutes: binary value at 0x0014
        //   - Hours: BCD values from 0x0015 to 0x0017
        // When the minutes counter reaches 60, the hour value is incremented.
        let time: [u8; 4] = self.intf.read_memory(0x0014).await?;
        let mins = time[0];
        let hours = utils::decode_bcd_value(u32::from_le_bytes([time[1], time[2], time[3], 0x00]));

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the stored faults.
    ///
    /// The faults are persisted in the EEPROM when turning off the machine.
    pub async fn query_faults(&mut self) -> Result<Fault, P::Error> {
        Fault::from_bits(self.intf.read_memory(0x000e).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the operating mode.
    pub async fn query_operating_mode(&mut self) -> Result<OperatingMode, P::Error> {
        OperatingMode::from_repr(self.intf.read_memory(0x0089).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program selection knob position.
    pub async fn query_program_selector(&mut self) -> Result<SelectorPosition, P::Error> {
        // The selector position is set from the value at 0x0132 after a short delay.
        // This value is also used to set the persistent program selection at 0x0001.
        SelectorPosition::from_repr(self.intf.read_memory(0x0071).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program type.
    ///
    /// The program type is set according to the program selector position.
    pub async fn query_program_type(&mut self) -> Result<ProgramType, P::Error> {
        // Program types are defined in a lookup table at address 0xa7a0.
        // The current type is determined by reading the value at 0x0001
        // to index into this table.
        ProgramType::from_repr(self.intf.read_memory(0x009e).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program temperature.
    ///
    /// The program temperature is set according to the program
    /// selector position and provided in `°C` (degrees Celsius).
    /// Some programs use a slightly lower temperature than selected.
    pub async fn query_program_temperature(&mut self) -> Result<u8, P::Error> {
        // Program temperatures are defined in a lookup table at address 0xa7b6.
        // The current temperature is determined by reading the value at 0x0001
        // to index into this table.
        Ok(self.intf.read_memory(0x009f).await?)
    }

    /// Queries the program options.
    ///
    /// The program options are typically set using the buttons on the front panel of the machine,
    /// although not all combinations can be selected.
    pub async fn query_program_options(&mut self) -> Result<ProgramOption, P::Error> {
        // The options are used to set the front panel indicator lights at 0x006b.
        ProgramOption::from_bits(self.intf.read_memory(0x0012).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Sets the program options.
    ///
    /// <div class="warning">
    /// The washing machine does not validate the chosen options. Caution is advised!
    /// </div>
    pub async fn set_program_options(&mut self, opts: ProgramOption) -> Result<(), P::Error> {
        Ok(self.intf.write_memory(0x0012, opts.bits()).await?)
    }

    /// Queries the program spin setting.
    pub async fn query_program_spin_setting(&mut self) -> Result<SpinSetting, P::Error> {
        // The spin setting is used to set the front panel indicator lights at 0x0065.
        SpinSetting::from_repr(self.intf.read_memory(0x0011).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Sets the program spin setting.
    ///
    /// The actual spin speed depends on the machine's programming options.
    pub async fn set_program_spin_setting(&mut self, speed: SpinSetting) -> Result<(), P::Error> {
        Ok(self.intf.write_memory(0x0011, speed as u8).await?)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        // Program phases are defined in a lookup table at address 0xe96f.
        // The phase is determined by reading the value at 0x0000 to index into this table,
        // keeping only the lower nibble of the resulting value.
        // This value is used to set the front panel indicator lights at 0x0068.
        ProgramPhase::from_repr(self.intf.read_memory(0x005e).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program locked state.
    ///
    /// The currently running program can be locked/unlocked by holding the _Start_ button.
    pub async fn query_program_locked(&mut self) -> Result<bool, P::Error> {
        let state: u8 = self.intf.read_memory(0x0005).await?;

        Ok((state & 0x04) != 0x00)
    }

    /// Queries the laundry load level.
    ///
    /// The load level ranges from 1 to 4 and is calculated by the machine during operation.
    /// For some program types, the maximum load level is limited to a lower value.
    pub async fn query_load_level(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x000a).await?)
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        // The active actuators at 0x0039 and 0x003a are
        // used to set the outputs at ports 5 and 6, respectively.
        Actuator::from_bits(self.intf.read_memory(0x0039).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the NTC thermistor resistance.
    ///
    /// The resistance in `Ω` (ohms) is calculated from the ADC voltage.
    pub async fn query_ntc_resistance(&mut self) -> Result<u32, P::Error> {
        let val: u8 = self.intf.read_memory(0x0179).await?;

        Ok(utils::ntc_resistance_from_adc(val))
    }

    /// Queries the current temperature sensed by the NTC thermistor and the target temperature.
    ///
    /// The temperature is provided in `°C` (degrees Celsius).
    pub async fn query_temperature(&mut self) -> Result<(u8, u8), P::Error> {
        // Temperatures are defined in a lookup table at address 0xde6a.
        let [target, current] = self.intf.read_memory(0x0138).await?;

        Ok((current, target))
    }

    /// Queries the current water level sensed by the analog pressure sensor and the target level.
    ///
    /// The water level is provided in `mmH₂O` (millimeters of water).
    pub async fn query_water_level(&mut self) -> Result<(u8, u8), P::Error> {
        // Target water levels are defined in a lookup table at address 0xe623.
        // The current target is determined by reading the value at 0x0000 to index into this table,
        // although it also seems to depend on the program temperature and load level.
        // In that case, the target is set from the lookup table at address 0xf042.
        let [current, target] = self.intf.read_memory(0x003b).await?;

        Ok((current, target))
    }

    /// Starts the selected program.
    ///
    /// As the program cannot be set using the diagnostic interface,
    /// the desired program has to be selected manually using the program selector.
    /// This function returns an error if no program has been chosen
    /// or a program is already running.
    pub async fn start_program(&mut self) -> Result<(), P::Error> {
        // Programs are managed by a state machine subroutine at 0xad38.
        // The current state is stored at 0x00a5. Known state values include:
        //   0x00: no program selected or running
        //   0x01: program selected and ready to start
        //   0x05: program running
        // Additional state values are utilized internally by the state machine.
        let state: u8 = self.intf.read_memory(0x00a5).await?;

        if state == 0x01 {
            Ok(self.intf.write_memory(0x00a5, 0x02u8).await?)
        } else {
            Err(Error::InvalidState)
        }
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
            PROP_PROGRAM_SELECTOR,
            PROP_PROGRAM_TYPE,
            PROP_PROGRAM_TEMPERATURE,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_SPIN_SETTING,
            PROP_PROGRAM_PHASE,
            PROP_PROGRAM_LOCKED,
            PROP_LOAD_LEVEL,
            PROP_ACTIVE_ACTUATORS,
            PROP_NTC_RESISTANCE,
            PROP_TEMPERATURE,
            PROP_WATER_LEVEL,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[
            ACTION_SET_PROGRAM_OPTIONS,
            ACTION_SET_PROGRAM_SPIN_SETTING,
            ACTION_START_PROGRAM,
        ]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_ROM_CODE => Ok(self.query_rom_code().await?.into()),
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            // Failure
            PROP_FAULTS => Ok(self.query_faults().await?.to_string().into()),
            // Operation
            PROP_OPERATING_MODE => Ok(self.query_operating_mode().await?.to_string().into()),
            PROP_PROGRAM_SELECTOR => Ok(self.query_program_selector().await?.to_string().into()),
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_PROGRAM_TEMPERATURE => Ok(self.query_program_temperature().await?.into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_SPIN_SETTING => {
                Ok(self.query_program_spin_setting().await?.to_string().into())
            }
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_PROGRAM_LOCKED => Ok(self.query_program_locked().await?.into()),
            PROP_LOAD_LEVEL => Ok(self.query_load_level().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_NTC_RESISTANCE => Ok(self.query_ntc_resistance().await?.into()),
            PROP_TEMPERATURE => Ok(self.query_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
            _ => Err(Error::UnknownProperty),
        }
    }

    async fn trigger_action(
        &mut self,
        action: &Action,
        param: Option<Value>,
    ) -> Result<(), P::Error> {
        match *action {
            ACTION_SET_PROGRAM_OPTIONS => match param {
                Some(Value::String(s)) => self.set_program_options(s.parse()?).await,
                _ => Err(Error::InvalidArgument),
            },
            ACTION_SET_PROGRAM_SPIN_SETTING => match param {
                Some(Value::String(s)) => self.set_program_spin_setting(s.parse()?).await,
                _ => Err(Error::InvalidArgument),
            },
            ACTION_START_PROGRAM => match param {
                None => self.start_program().await,
                _ => Err(Error::InvalidArgument),
            },
            _ => Err(Error::UnknownAction),
        }
    }
}

impl<P> private::Sealed for WashingMachine<P> {}
