//! Device support for W 8xx/9xx series washing machines.
//!
//! Supports appliances with software ID 132, which typically use an EDPW 122 board or similar.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, Device, DeviceKind, Error, Fault, Interface, Property, PropertyKind, Result, Value,
    private, utils,
};
use alloc::{boxed::Box, string::ToString};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::{str, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        132
    };
}
pub(super) use compatible_software_ids;

const PROP_OPERATING_TIME: Property = Property {
    kind: PropertyKind::General,
    id: "operating_time",
    name: "Operating Time",
    unit: None,
};
const PROP_FAULT_F1: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f1",
    name: "F1: Water Level Switch",
    unit: None,
};
const PROP_FAULT_F2: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f2",
    name: "F2: NTC Thermistor",
    unit: None,
};
const PROP_FAULT_F3: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f3",
    name: "F3: Heater",
    unit: None,
};
const PROP_FAULT_F4: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f4",
    name: "F4: Tachometer",
    unit: None,
};
const PROP_FAULT_F5: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f5",
    name: "F5: Detergent Overdose",
    unit: None,
};
const PROP_FAULT_F6: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f6",
    name: "F6: Water Inlet",
    unit: None,
};
const PROP_FAULT_F7: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f7",
    name: "F7: Drainage",
    unit: None,
};
const PROP_FAULT_F8: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f8",
    name: "F8: Final Spin Speed",
    unit: None,
};
const PROP_FAULT_F9: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f9",
    name: "F9: EEPROM",
    unit: None,
};
const PROP_SELECTED_PROGRAM: Property = Property {
    kind: PropertyKind::Operation,
    id: "selected_program",
    name: "Selected Program",
    unit: None,
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
    unit: None,
};
const PROP_PROGRAM_PHASE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_phase",
    name: "Program Phase",
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
const PROP_TARGET_TEMPERATURE: Property = Property {
    kind: PropertyKind::Io,
    id: "target_temperature",
    name: "Target Temperature",
    unit: Some("°C"),
};
const PROP_WATER_LEVEL: Property = Property {
    kind: PropertyKind::Io,
    id: "water_level",
    name: "Water Level",
    unit: None,
};
const PROP_TACHOMETER_SPEED: Property = Property {
    kind: PropertyKind::Io,
    id: "tachometer_speed",
    name: "Tachometer Speed",
    unit: Some("rpm"),
};

/// Washing machine fault code.
///
/// Each code represents a specific fault condition that can occur in the machine.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum FaultCode {
    /// Level switch (digital pressure sensor) fault.
    LevelSwitch,
    /// NTC thermistor (temperature sensor) fault.
    NtcThermistor,
    /// Heater fault.
    Heater,
    /// Tachometer generator fault.
    Tachometer,
    /// Detergent overdose fault.
    DetergentOverdose,
    /// Water inlet fault.
    WaterInlet,
    /// Drainage fault.
    Drainage,
    /// Final spin cycle speed too low (< 400 rpm) fault.
    FinalSpinSpeed,
    /// EEPROM fault.
    Eeprom,
}

/// Washing machine program.
///
/// Each variant represents a position of the machine's program selector knob.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Program {
    /// Finish position (no program selected).
    Finish,
    /// Cottons program, 95 °C.
    Cottons95,
    /// Cottons program, 75 °C.
    Cottons75,
    /// Cottons program, 60 °C.
    Cottons60,
    /// Cottons program, 50 °C.
    Cottons50,
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
    /// Delicates program, 60 °C.
    Delicates60,
    /// Delicates program, 50 °C.
    Delicates50,
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
        /// Softener compartment actuator.
        const Softener = 0x0002;
        /// Pre-wash compartment actuator.
        const PreWash = 0x0004;
        /// Main wash compartment actuator.
        const MainWash = 0x0008;
        /// Drain pump actuator.
        const DrainPump = 0x0010;
        /// Warm water actuator.
        const WarmWater = 0x0020;
        /// Reverse relay actuator.
        const Reverse = 0x2000;
        /// Motor field switch relay actuator.
        const FieldSwitch = 0x4000;
        /// Heater actuator.
        const Heater = 0x8000;
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
/// use freemdu::device::{Device, id132::WashingMachine};
///
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut machine = WashingMachine::connect(&mut port).await?;
///
/// println!("Selected program: {}", machine.query_selected_program().await?);
/// println!("Program options: {}", machine.query_program_options().await?);
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
        intf.unlock_read_access(0x15a8).await?;
        intf.unlock_full_access(0x703d).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    /// It is internally stored in minutes and hours but only the hours are displayed in the service mode.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // The current time is stored as follows:
        //   - Minutes: binary value at 0x0012
        //   - Hours: BCD values from 0x0013 to 0x0015
        // When the minutes counter reaches 60, the hour value is incremented.
        let time: [u8; 4] = self.intf.read_memory(0x0012).await?;
        let mins = time[0];
        let hours = utils::decode_bcd_value(u32::from_le_bytes([time[1], time[2], time[3], 0x00]));

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
        let mut query =
            async |active: (u16, u8), stored: Option<(u16, u8)>| -> Result<Fault, P::Error> {
                let val: u8 = self.intf.read_memory(active.0.into()).await?;

                if (val & active.1) != 0x00 {
                    Ok(Fault::Active(None))
                } else if let Some(stored) = stored {
                    let val: u8 = self.intf.read_memory(stored.0.into()).await?;

                    if (val & stored.1) != 0x00 {
                        Ok(Fault::Stored(None))
                    } else {
                        Ok(Fault::Ok)
                    }
                } else {
                    Ok(Fault::Ok)
                }
            };

        match code {
            // Detergent overdose only has two states (ok/active)
            // That fault is stored but doesn't have a dedicated bit for the active state
            FaultCode::LevelSwitch => query((0x0061, 0x02), Some((0x000e, 0x01))),
            FaultCode::NtcThermistor => query((0x0061, 0x04), Some((0x000e, 0x02))),
            FaultCode::Heater => query((0x0004, 0x20), Some((0x000e, 0x04))),
            FaultCode::Tachometer => query((0x007a, 0x02), Some((0x000e, 0x08))),
            FaultCode::DetergentOverdose => query((0x000e, 0x10), None),
            FaultCode::WaterInlet => query((0x0004, 0x02), Some((0x000e, 0x20))),
            FaultCode::Drainage => query((0x0004, 0x04), Some((0x000e, 0x40))),
            FaultCode::FinalSpinSpeed => query((0x0037, 0x10), Some((0x000e, 0x80))),
            FaultCode::Eeprom => query((0x0131, 0x0c), Some((0x000f, 0x01))),
        }
        .await
    }

    /// Queries the selected program.
    pub async fn query_selected_program(&mut self) -> Result<Program, P::Error> {
        // The selected program is set from the value at 0x0124 after a short delay.
        // This value is also used to set the persistent program value at 0x0001.
        Program::from_repr(self.intf.read_memory(0x0114).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program options.
    ///
    /// The program options are set using the buttons on the front panel of the machine.
    pub async fn query_program_options(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x006c).await?)
    }

    /// Queries the program spin setting.
    pub async fn query_program_spin_setting(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x006d).await?)
    }

    /// Queries the program spin speed.
    pub async fn query_program_spin_speed(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x0059).await?)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        // Program phases are defined in a lookup table at address 0xe753.
        // The phase is determined by reading the value at 0x0000 to index into this table,
        // keeping only the lower nibble of the resulting value.
        // This value is used to set the front panel indicator lights at 0x0038 and 0x0039.
        ProgramPhase::from_repr(self.intf.read_memory(0x001c).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        // The active actuators at 0x003a and 0x003b are
        // used to set the outputs at ports 5 and 6, respectively.
        Actuator::from_bits(self.intf.read_memory(0x003a).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the NTC thermistor resistance.
    ///
    /// The resistance in `Ω` (ohms) is calculated from the ADC voltage.
    pub async fn query_ntc_resistance(&mut self) -> Result<u32, P::Error> {
        let val: u8 = self.intf.read_memory(0x0021).await?;

        Ok(utils::ntc_resistance_from_adc(val))
    }

    /// Queries the target temperature.
    ///
    /// The temperature is provided in `°C` (degrees Celsius).
    pub async fn query_target_temperature(&mut self) -> Result<u8, P::Error> {
        // The ADC readings from the NTC thermistor are
        // not converted into a temperature value in °C.
        // For this reason, the readings are compared with static thresholds
        // that are defined in a lookup table at 0xadca.
        // The threshold is selected based on the target temperature index.
        const TEMPERATURES: [u8; 15] = [90, 21, 27, 32, 34, 37, 47, 57, 72, 77, 80, 82, 85, 86, 65];
        let target: u8 = self.intf.read_memory(0x005c).await?;

        TEMPERATURES
            .get(target as usize)
            .copied()
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the current water level sensed by the digital pressure sensor and the target level.
    ///
    /// The water level is provided as a discrete value and ranges from 0 to 3.
    pub async fn query_water_level(&mut self) -> Result<(u8, u8), P::Error> {
        // The current water level is determined by a subroutine at 0xbee0.
        // Target water levels are defined in a lookup table at address 0xe57d,
        // which is indexed by the value at 0x0000 and stored in 0x005b.
        // The lowest 3 bytes of this value are then used to set the target water level.
        let [current, target] = self.intf.read_memory(0x003c).await?;

        Ok((current, target))
    }

    /// Queries the current speed sensed by the tachometer generator and the target speed.
    ///
    /// The speed in `rpm` (revolutions per minute) is only provided
    /// by the machine during the spin phase.
    pub async fn query_tachometer_speed(&mut self) -> Result<(u16, u16), P::Error> {
        // The target speed is selected from a lookup table at 0xc71d,
        // based on the value of the memory at 0x006e multiplied by 2.
        // Motor control calculations are performed in a subroutine at 0xc747.
        let speed: [u8; 5] = self.intf.read_memory(0x006f).await?;
        let target_raw = u16::from_le_bytes([speed[0], speed[1]]);
        let current_raw = u32::from_le_bytes([speed[2], speed[3], speed[4], 0x00]);
        let current =
            utils::rpm_from_motor_speed(current_raw).ok_or(Error::UnexpectedMemoryValue)?;
        let target = utils::rpm_from_motor_speed(u32::from(target_raw))
            .ok_or(Error::UnexpectedMemoryValue)?;

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

    fn software_id(&self) -> u16 {
        self.software_id
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::WashingMachine
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_OPERATING_TIME,
            PROP_FAULT_F1,
            PROP_FAULT_F2,
            PROP_FAULT_F3,
            PROP_FAULT_F4,
            PROP_FAULT_F5,
            PROP_FAULT_F6,
            PROP_FAULT_F7,
            PROP_FAULT_F8,
            PROP_FAULT_F9,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_SPIN_SETTING,
            PROP_PROGRAM_SPIN_SPEED,
            PROP_PROGRAM_PHASE,
            PROP_ACTIVE_ACTUATORS,
            PROP_NTC_RESISTANCE,
            PROP_TARGET_TEMPERATURE,
            PROP_WATER_LEVEL,
            PROP_TACHOMETER_SPEED,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            // Fault
            PROP_FAULT_F1 => Ok(self.query_fault(FaultCode::LevelSwitch).await?.into()),
            PROP_FAULT_F2 => Ok(self.query_fault(FaultCode::NtcThermistor).await?.into()),
            PROP_FAULT_F3 => Ok(self.query_fault(FaultCode::Heater).await?.into()),
            PROP_FAULT_F4 => Ok(self.query_fault(FaultCode::Tachometer).await?.into()),
            PROP_FAULT_F5 => Ok(self.query_fault(FaultCode::DetergentOverdose).await?.into()),
            PROP_FAULT_F6 => Ok(self.query_fault(FaultCode::WaterInlet).await?.into()),
            PROP_FAULT_F7 => Ok(self.query_fault(FaultCode::Drainage).await?.into()),
            PROP_FAULT_F8 => Ok(self.query_fault(FaultCode::FinalSpinSpeed).await?.into()),
            PROP_FAULT_F9 => Ok(self.query_fault(FaultCode::Eeprom).await?.into()),
            // Operation
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_SPIN_SETTING => {
                Ok(self.query_program_spin_setting().await?.to_string().into())
            }
            PROP_PROGRAM_SPIN_SPEED => {
                Ok(self.query_program_spin_speed().await?.to_string().into())
            }
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_NTC_RESISTANCE => Ok(self.query_ntc_resistance().await?.into()),
            PROP_TARGET_TEMPERATURE => Ok(self.query_target_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
            PROP_TACHOMETER_SPEED => Ok(self.query_tachometer_speed().await?.into()),
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
