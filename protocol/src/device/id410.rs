//! Device support for W 3xx series washing machines.
//!
//! Supports appliances with software ID 410, including the Miele Novotronic W 307.
//!
//! This implementation is intentionally read-only. Only the read access key is used and no
//! actions are exposed.

use crate::device::{
    Action, Device, DeviceKind, Error, Interface, Property, PropertyKind, Result, Value, private,
};
use alloc::boxed::Box;
use alloc::{format, string::ToString};
use embedded_io_async::{Read, Write};

macro_rules! compatible_software_ids {
    () => {
        410
    };
}
pub(super) use compatible_software_ids;

// These are marked as Operation properties intentionally: the Home firmware currently publishes
// only Operation properties via MQTT/Home Assistant. Keeping them here avoids any ID410-specific
// changes in the Home firmware while retaining the read-only sensors verified on the W307.
const PROP_CYCLE_STATUS: Property = Property {
    kind: PropertyKind::Operation,
    id: "cycle_status",
    name: "Cycle Status",
    unit: None,
};
const PROP_PROGRAM_RUNNING: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_running",
    name: "Program Running",
    unit: None,
};
const PROP_PROGRAM_FINISHED: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_finished",
    name: "Program Finished",
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
const PROP_PROGRAM_PHASE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_phase",
    name: "Program Phase",
    unit: None,
};
const PROP_PROGRAM_TEMPERATURE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_temperature",
    name: "Selected Temperature",
    unit: Some("°C"),
};
const PROP_TEMPERATURE: Property = Property {
    kind: PropertyKind::Operation,
    id: "temperature",
    name: "Drum Temperature",
    unit: Some("°C"),
};
const PROP_TARGET_TEMPERATURE: Property = Property {
    kind: PropertyKind::Operation,
    id: "target_temperature",
    name: "Target Temperature",
    unit: Some("°C"),
};
const PROP_WATER_LEVEL: Property = Property {
    kind: PropertyKind::Operation,
    id: "water_level",
    name: "Water Level (Raw)",
    unit: None,
};
const PROP_WATER_LEVEL_TARGET: Property = Property {
    kind: PropertyKind::Operation,
    id: "water_level_target",
    name: "Target Water Level (Raw)",
    unit: None,
};
const PROP_MOTOR_PWM_DUTY_CYCLE: Property = Property {
    kind: PropertyKind::Operation,
    id: "motor_pwm_duty_cycle",
    name: "Motor PWM Duty Cycle",
    unit: Some("%"),
};
const PROP_ACTIVE_ACTUATORS_RAW: Property = Property {
    kind: PropertyKind::Operation,
    id: "active_actuators_raw",
    name: "Active Actuators (Raw)",
    unit: None,
};

/// Washing machine device implementation for software ID 410.
#[derive(Debug)]
pub struct WashingMachine<P>
where
    P: Read + Write,
{
    intf: Interface<P>,
    software_id: u16,
}

impl<P> WashingMachine<P>
where
    P: Read + Write,
{
    pub(crate) async fn initialize(
        mut intf: Interface<P>,
        software_id: u16,
    ) -> Result<Self, P::Error> {
        // Confirmed read key for the W307 / software ID 410. Deliberately do not request full
        // access: this device implementation must remain read-only.
        intf.unlock_read_access(0x43ea).await?;
        Ok(Self { intf, software_id })
    }

    async fn query_operating_state_raw(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x00cd).await?)
    }

    /// Queries the current machine/cycle status.
    pub async fn query_cycle_status(&mut self) -> Result<&'static str, P::Error> {
        Ok(match self.query_operating_state_raw().await? {
            0 => "Door open",
            1 => "Ready",
            2 => "Running",
            3 => "Finished",
            4 => "Service programming",
            5 => "Customer programming",
            6 => "Service",
            _ => "Unknown",
        })
    }

    /// Queries whether a washing program is currently running.
    pub async fn query_program_running(&mut self) -> Result<bool, P::Error> {
        Ok(self.query_operating_state_raw().await? == 2)
    }

    /// Queries whether the current washing program has finished.
    pub async fn query_program_finished(&mut self) -> Result<bool, P::Error> {
        Ok(self.query_operating_state_raw().await? == 3)
    }

    /// Queries the program selected with the front-panel program selector.
    pub async fn query_selected_program(&mut self) -> Result<&'static str, P::Error> {
        let value: u8 = self.intf.read_memory(0x00b5).await?;
        Ok(match value {
            0 => "Finish",
            1 => "Cottons 95 °C",
            2 => "Cottons 75 °C",
            3 => "Cottons 60 °C",
            4 => "Cottons 40 °C",
            5 => "Cottons 30 °C",
            6 => "Minimum iron 60 °C",
            7 => "Minimum iron 50 °C",
            8 => "Minimum iron 40 °C",
            9 => "Minimum iron 30 °C",
            10 => "Drain/Spin",
            11 => "Separate rinse",
            12 => "Starch",
            13 => "Mixed wash 40 °C",
            14 => "Quick wash 40 °C",
            15 => "Woolens cold",
            16 => "Woolens 30 °C",
            17 => "Woolens 40 °C",
            18 => "Silks 30 °C",
            19 => "Delicates cold",
            20 => "Delicates 30 °C",
            21 => "Delicates 40 °C",
            _ => "Unknown",
        })
    }

    /// Queries the general category of the selected program.
    pub async fn query_program_type(&mut self) -> Result<&'static str, P::Error> {
        let value: u8 = self.intf.read_memory(0x00de).await?;
        Ok(match value {
            0x00 => "None",
            0x01 => "Cottons",
            0x02 => "Minimum iron",
            0x03 => "Delicates",
            0x04 => "Woolens",
            0x05 => "Quick wash",
            0x06 => "Starch",
            0x07 => "Drain/Spin",
            0x09 => "Separate rinse",
            0x0a => "Mixed wash",
            0x0b => "Silks",
            _ => "Unknown",
        })
    }

    /// Queries the current phase of the washing program.
    pub async fn query_program_phase(&mut self) -> Result<&'static str, P::Error> {
        // 0, 4 and 13 were observed directly on ID410. The remaining labels follow the closely
        // related ID360 profile and are intentionally kept read-only until observed on ID410.
        let value: u8 = self.intf.read_memory(0x00a2).await?;
        Ok(match value {
            0 => "Idle",
            1 => "Delayed start",
            2 => "Soak/Pre-wash 1",
            3 => "Soak/Pre-wash 2",
            4 => "Main wash",
            5 => "Rinse 1",
            6 => "Rinse 2",
            7 => "Rinse 3",
            8 => "Rinse 4",
            9 => "Rinse 5",
            10 => "Rinse hold",
            11 => "Drain",
            12 => "Final spin",
            13 => "Anti-crease/Finish",
            _ => "Unknown",
        })
    }

    /// Queries the temperature selected for the current program, in degrees Celsius.
    pub async fn query_program_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x00df).await?)
    }

    /// Queries the current drum temperature, in degrees Celsius.
    pub async fn query_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x0136).await?)
    }

    /// Queries the current target drum temperature, in degrees Celsius.
    pub async fn query_target_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x0135).await?)
    }

    /// Queries the raw current water-level value.
    pub async fn query_water_level(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x007f).await?)
    }

    /// Queries the raw target water-level value.
    pub async fn query_water_level_target(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x0080).await?)
    }

    /// Queries the drum-motor PWM duty cycle, in percent.
    pub async fn query_motor_pwm_duty_cycle(&mut self) -> Result<u8, P::Error> {
        let duty: u8 = self.intf.read_memory(0x0280).await?;
        Ok((u16::from(duty) * 100 / 0xff).try_into()?)
    }

    /// Queries the raw active-actuator bit field.
    pub async fn query_active_actuators_raw(&mut self) -> Result<u16, P::Error> {
        Ok(self.intf.read_memory(0x007d).await?)
    }
}

#[async_trait::async_trait(?Send)]
impl<P> Device<P> for WashingMachine<P>
where
    P: Read + Write,
{
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
            PROP_CYCLE_STATUS,
            PROP_PROGRAM_RUNNING,
            PROP_PROGRAM_FINISHED,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_TYPE,
            PROP_PROGRAM_PHASE,
            PROP_PROGRAM_TEMPERATURE,
            PROP_TEMPERATURE,
            PROP_TARGET_TEMPERATURE,
            PROP_WATER_LEVEL,
            PROP_WATER_LEVEL_TARGET,
            PROP_MOTOR_PWM_DUTY_CYCLE,
            PROP_ACTIVE_ACTUATORS_RAW,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            PROP_CYCLE_STATUS => Ok(self.query_cycle_status().await?.to_string().into()),
            PROP_PROGRAM_RUNNING => Ok(self.query_program_running().await?.into()),
            PROP_PROGRAM_FINISHED => Ok(self.query_program_finished().await?.into()),
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_PROGRAM_TEMPERATURE => Ok(self.query_program_temperature().await?.into()),
            PROP_TEMPERATURE => Ok(self.query_temperature().await?.into()),
            PROP_TARGET_TEMPERATURE => Ok(self.query_target_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
            PROP_WATER_LEVEL_TARGET => Ok(self.query_water_level_target().await?.into()),
            PROP_MOTOR_PWM_DUTY_CYCLE => Ok(self.query_motor_pwm_duty_cycle().await?.into()),
            PROP_ACTIVE_ACTUATORS_RAW => {
                Ok(format!("0x{:04x}", self.query_active_actuators_raw().await?).into())
            }
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

impl<P> private::Sealed for WashingMachine<P> where P: Read + Write {}
