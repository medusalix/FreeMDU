//! Experimental read-only support for the T4223C, software ID 498.
//! See docs/id498.md for evidence, address limits and interpretation caveats.
use crate::device::{
    Action, Device, DeviceKind, Error, Interface, Property, PropertyKind, Result, Value, private,
};
use alloc::{boxed::Box, format, string::ToString};
use embedded_io_async::{Read, Write};

/// Confirmed read-access key. Monitoring never unlocks full access.
pub const READ_KEY: u16 = 0x2b2c;
macro_rules! compatible_software_ids {
    () => {
        498
    };
}
pub(super) use compatible_software_ids;

/// Decode observed selector positions; unsupported positions remain unknown.
#[must_use]
pub fn program(raw: u8) -> &'static str {
    match raw {
        15 => "Ende",
        14 => "Koch/Bunt Schranktrocken+",
        1 => "Koch/Bunt Schranktrocken / Schonen",
        3 => "Koch/Bunt Buegelfeucht",
        2 => "Koch/Bunt Mangelfeucht",
        6 => "Glaetten",
        7 => "Finish Wolle",
        8 => "Pflegeleicht Schranktrocken+",
        9 => "Pflegeleicht Schranktrocken",
        13 => "Pflegeleicht Schranktrocken / Schonen",
        12 => "Pflegeleicht Buegelfeucht",
        4 => "20 min warm",
        5 => "15 min kalt",
        _ => "Unknown",
    }
}
/// Require agreement between all three observed door bytes.
#[must_use]
pub fn door(raw: [u8; 3]) -> &'static str {
    match raw {
        [0, 0, 0] => "Closed",
        [1, 1, 1] => "Open",
        _ => "Unknown",
    }
}
/// Program marker, not actual drum movement or a completion indication.
#[must_use]
pub fn running(raw: u8) -> &'static str {
    match raw {
        0xaa => "Running",
        0x55 => "Not running",
        _ => "Unknown",
    }
}
/// Raw marker labels: physical movement and direction remain unverified.
#[must_use]
pub fn motion(raw: u8) -> &'static str {
    match raw {
        0 => "Marker 0",
        1 => "Marker 1",
        2 => "Marker 2",
        _ => "Unknown",
    }
}
/// Stateless phase interpretation cannot distinguish all inactive states.
#[must_use]
pub fn phase(selector: u8, run: u8, motor: u8) -> &'static str {
    match (selector, run, motor) {
        (_, 0xaa, 0..=2) => "Program active",
        (15, 0x55, 0) => "Selector at Ende",
        (_, 0x55, 0..=2) => "Inactive / waiting / interrupted / post-run",
        _ => "Unknown",
    }
}
/// A closed door with an inactive marker does not prove readiness or completion.
#[must_use]
pub fn observed_phase(selector: u8, run: u8, motor: u8, triplet: [u8; 3]) -> &'static str {
    match (door(triplet), run) {
        ("Open", 0x55) => "Door open / inactive",
        ("Closed", _) => phase(selector, run, motor),
        _ => "Unknown",
    }
}
/// Properties exported by the generic TUI and Home Assistant integration.
pub const PROPERTIES: &[Property] = &[
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_program",
        name: "Dryer Selected program",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_door",
        name: "Dryer Door",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_run_state",
        name: "Dryer Program state (experimental)",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_motion",
        name: "Dryer Direction marker (raw)",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_phase",
        name: "Dryer Observed phase (experimental)",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_program_raw",
        name: "Dryer Program marker raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_door_raw",
        name: "Dryer Door triplet raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_run_raw",
        name: "Dryer Run marker raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_flags_raw",
        name: "Dryer Flags 026a/027e raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_post_run_raw",
        name: "Dryer Marker 0260 raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_motion_raw",
        name: "Dryer Motor marker 027e raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_transition_raw",
        name: "Dryer Transition marker 027f raw",
        unit: None,
    },
    Property {
        kind: PropertyKind::Operation,
        id: "dryer_software_id",
        name: "Dryer Software ID",
        unit: None,
    },
];
/// Three sequential reads, not an atomic snapshot of appliance memory.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    /// Memory block 00b0.
    pub program: [u8; 16],
    /// Memory block 0260.
    pub door: [u8; 16],
    /// Memory block 0270.
    pub run: [u8; 16],
}
impl Snapshot {
    /// Read only the validated memory window.
    pub async fn read<P: Read + Write>(intf: &mut Interface<P>) -> Result<Self, P::Error> {
        Ok(Self {
            program: intf.read_memory(0x00b0).await?,
            door: intf.read_memory(0x0260).await?,
            run: intf.read_memory(0x0270).await?,
        })
    }
    /// Decode properties while preserving unknown marker values.
    pub fn value<E>(&self, prop: &Property) -> Result<Value, E> {
        Ok(match prop.id {
            "dryer_program" => program(self.program[6]).to_string().into(),
            "dryer_door" => door([self.door[5], self.door[6], self.door[7]])
                .to_string()
                .into(),
            "dryer_run_state" => running(self.run[0]).to_string().into(),
            "dryer_motion" => motion(self.run[14]).to_string().into(),
            "dryer_phase" => observed_phase(
                self.program[6],
                self.run[0],
                self.run[14],
                [self.door[5], self.door[6], self.door[7]],
            )
            .to_string()
            .into(),
            "dryer_program_raw" => format!("0x{:02x}", self.program[6]).into(),
            "dryer_door_raw" => format!(
                "{:02x} {:02x} {:02x}",
                self.door[5], self.door[6], self.door[7]
            )
            .into(),
            "dryer_run_raw" => format!("0x{:02x}", self.run[0]).into(),
            "dryer_flags_raw" => format!("{:02x} {:02x}", self.door[10], self.run[14]).into(),
            "dryer_post_run_raw" => format!("0x{:02x}", self.door[0]).into(),
            "dryer_motion_raw" => format!("0x{:02x}", self.run[14]).into(),
            "dryer_transition_raw" => format!("0x{:02x}", self.run[15]).into(),
            "dryer_software_id" => 498u32.into(),
            _ => return Err(Error::UnknownProperty),
        })
    }
}
/// Read-only dryer profile with no appliance actions.
#[derive(Debug)]
pub struct TumbleDryer<P> {
    intf: Interface<P>,
}
impl<P: Read + Write> TumbleDryer<P> {
    pub(crate) async fn initialize(mut intf: Interface<P>, _id: u16) -> Result<Self, P::Error> {
        intf.unlock_read_access(READ_KEY).await?;
        Ok(Self { intf })
    }
}
#[async_trait::async_trait(?Send)]
impl<P: Read + Write> Device<P> for TumbleDryer<P> {
    async fn connect(port: P) -> Result<Self, P::Error> {
        let mut intf = Interface::new(port);
        let id = intf.query_software_id().await?;
        if id != 498 {
            return Err(Error::UnknownSoftwareId(id));
        }
        Self::initialize(intf, id).await
    }
    fn software_id(&self) -> u16 {
        498
    }
    fn kind(&self) -> DeviceKind {
        DeviceKind::TumbleDryer
    }
    fn properties(&self) -> &'static [Property] {
        PROPERTIES
    }
    fn actions(&self) -> &'static [Action] {
        &[]
    }
    fn interface(&mut self) -> &mut Interface<P> {
        &mut self.intf
    }
    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        if !PROPERTIES.contains(prop) {
            return Err(Error::UnknownProperty);
        }
        Snapshot::read(&mut self.intf).await?.value(prop)
    }
    async fn trigger_action(&mut self, _: &Action, _: Option<&str>) -> Result<(), P::Error> {
        Err(Error::UnknownAction)
    }
}
impl<P: Read + Write> private::Sealed for TumbleDryer<P> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labelled_dumps() {
        let cases: &[(&[u8], u8, &str, &str)] = &[
            (
                include_bytes!("../../tests/fixtures/id498/0.bin"),
                15,
                "Closed",
                "Not running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/1.bin"),
                14,
                "Closed",
                "Not running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/2.bin"),
                8,
                "Closed",
                "Not running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/3.bin"),
                4,
                "Closed",
                "Not running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/4.bin"),
                5,
                "Closed",
                "Not running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/5.bin"),
                5,
                "Open",
                "Not running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/6.bin"),
                5,
                "Closed",
                "Running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/7.bin"),
                4,
                "Closed",
                "Running",
            ),
            (
                include_bytes!("../../tests/fixtures/id498/8.bin"),
                15,
                "Closed",
                "Not running",
            ),
        ];
        for (b, selector, d, run) in cases {
            let snap = Snapshot {
                program: b[0xb0..0xc0].try_into().unwrap(),
                door: b[0x260..0x270].try_into().unwrap(),
                run: b[0x270..0x280].try_into().unwrap(),
            };
            assert_eq!(snap.program[6], *selector);
            assert_ne!(program(*selector), "Unknown");
            assert_eq!(door([snap.door[5], snap.door[6], snap.door[7]]), *d);
            assert_eq!(running(snap.run[0]), *run);
            for p in PROPERTIES {
                assert!(snap.value::<core::convert::Infallible>(p).is_ok());
            }
        }
    }
    #[tokio::test]
    async fn connection_only_unlocks_reads() {
        use alloc::collections::VecDeque;
        let mut port = VecDeque::from([0x00, 0xf2, 0x01, 0xf3, 0x00]);
        {
            let dev = crate::device::connect(&mut port).await.unwrap();
            assert_eq!(dev.software_id(), 498);
            assert_eq!(dev.kind(), DeviceKind::TumbleDryer);
            assert!(dev.actions().is_empty());
        }
        assert_eq!(port, [0x11, 0, 0, 2, 0x13, 0, 0x20, 0x2c, 0x2b, 0, 0x77]);
    }
    #[test]
    fn validated_interruption_and_selectors() {
        assert_eq!(
            observed_phase(5, 0x55, 0, [1, 1, 1]),
            "Door open / inactive"
        );
        assert_eq!(
            observed_phase(5, 0x55, 0, [0, 0, 0]),
            "Inactive / waiting / interrupted / post-run"
        );
        assert_eq!(observed_phase(5, 0xaa, 1, [0, 0, 0]), "Program active");
        assert_eq!(observed_phase(5, 0xaa, 1, [1, 1, 1]), "Unknown");
        assert_eq!(observed_phase(5, 0x55, 0, [0, 1, 0]), "Unknown");
        for (raw, label) in [
            (1, "Koch/Bunt Schranktrocken / Schonen"),
            (3, "Koch/Bunt Buegelfeucht"),
            (2, "Koch/Bunt Mangelfeucht"),
            (6, "Glaetten"),
            (7, "Finish Wolle"),
        ] {
            assert_eq!(program(raw), label);
        }
    }
    #[test]
    fn uncertainty_and_post_run() {
        assert_eq!(door([0, 1, 0]), "Unknown");
        assert_eq!(program(255), "Unknown");
        assert_eq!(running(0), "Unknown");
        assert_eq!(motion(3), "Unknown");
        assert_eq!(phase(5, 0xaa, 0), "Program active");
        assert_eq!(
            phase(5, 0x55, 1),
            "Inactive / waiting / interrupted / post-run"
        );
        assert_eq!(
            phase(5, 0x55, 0),
            "Inactive / waiting / interrupted / post-run"
        );
    }
}
