//! Common definitions and types for newer devices.
//!
//! Older devices define their own device-specific types instead.

use strum::{Display, EnumString, FromRepr, VariantNames};

/// Washing machine program.
///
/// This enum is marked `#[non_exhaustive]` to allow for future variants.
#[non_exhaustive]
#[derive(FromRepr, Display, EnumString, VariantNames, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum WashingProgram {
    /// No program.
    None,
    /// Cottons program.
    Cottons,
    /// Coloreds program.
    Coloreds,
    /// Minimum iron program.
    MinimumIron,
    /// Delicates program.
    Delicates,
    /// Synthetics program.
    Synthetics,
    /// Mixed wash program.
    MixedWash,
    /// Quick wash program.
    QuickWash,
    /// Woolens program.
    Woolens,
    /// Silks program.
    Silks,
    /// Thermal disinfection program (85 °C, 15 min).
    ThermalDisinfection85Deg15Min,
    /// Chemo/thermal disinfection program (70 °C, 10 min).
    ChemoThermalDisinfection70Deg10Min,
    /// Short mops program.
    MopsShort,
    /// Floor polisher pads program.
    FloorPolisherPads,
    /// Hygiene program.
    Hygiene,
    /// Undergarment sanitization program (high temperature).
    UndergarmentSanitizationHigh,
    /// Undergarment sanitization program (low temperature).
    UndergarmentSanitizationLow,
    /// Starch program.
    Starch,
    /// Separate rinse program.
    SeparateRinse,
    /// Drain program.
    Drain,
    /// Spin program.
    Spin,
    /// Drain spin program.
    DrainSpin,
    /// Curtains program.
    Curtains,
    /// Shirts program.
    Shirts,
    /// Denim jeans program.
    DenimJeans,
    /// Anoraks program.
    Anoraks,
    /// Skirts program.
    Skirts,
    /// Proof program.
    Proof,
    /// Sneakers program.
    Sneakers,
    /// Sportswear program.
    Sportswear,
    /// Diapers program.
    Diapers,
    /// Automatic program.
    Automatic,
    /// Sleeping bags program.
    SleepingBags,
    /// Table linens program.
    TableLinens,
    /// Kitchen linens program.
    KitchenLinens,
    /// Towelling program.
    Towelling,
    /// Blankets program.
    Blankets,
    /// Outdoor jackets program.
    OutdoorJackets,
    /// Short program.
    Short,
    /// Pillows program.
    Pillows,
    /// Special program 1.
    SpecialProgram1,
    /// Special program 2.
    SpecialProgram2,
    /// Special program 3.
    SpecialProgram3,
    /// Intensive wash program.
    IntensiveWash,
    /// Towels program.
    Towels,
    /// Cool air program (washer-dryer).
    CoolAir,
    /// Warm air program (washer-dryer).
    WarmAir,
    /// Gentle smoothing program (washer-dryer).
    GentleSmoothing,
    /// Rinse out lint program (washer-dryer).
    RinseOutLint,
    /// Express program.
    Express,
    /// Dark garments program.
    DarkGarments,
    /// Cottons program (alternative).
    CottonsAlt,
    /// Separate rinse/starch program.
    SeparateRinseStarch,
    /// New textiles program.
    NewTextiles,
    /// Chemo/thermal disinfection program (60 °C, 20 min).
    ChemoThermalDisinfection60Deg20Min,
    /// Soak program.
    Soak,
    /// Extra white program.
    ExtraWhite,
    /// Stains program.
    Stains,
    /// Casual program.
    Casual,
    /// Timed drying program (washer-dryer).
    TimedDrying,
    /// Chemo/thermal disinfection program (40 °C, 30 min).
    ChemoThermalDisinfection40Deg30Min,
    /// Bed linens program.
    BedLinens,
    /// Flour program.
    Flour,
    /// Flour plus program.
    FlourPlus,
    /// Grease program.
    Grease,
    /// Grease plus program.
    GreasePlus,
    /// Towels plus program.
    TowelsPlus,
    /// Capes program.
    Capes,
    /// MRSA bacteria plus program.
    MrsaBacteriaPlus,
    /// Cottons hygiene program.
    CottonsHygiene,
    /// Minimum iron hygiene program.
    MinimumIronHygiene,
    /// Garden chair cushions program.
    GardenChairCushions,
    /// Animal blankets program.
    AnimalBlankets,
    /// Suits program.
    Suits,
    /// Outerwear program.
    Outerwear,
    /// Steam smoothing program.
    SteamSmoothing,
    /// Refresh program.
    Refresh,
    /// Sports shoes program.
    SportsShoes,
    /// Stuffed animals program.
    StuffedAnimals,
    /// Standard program.
    Standard,
    /// Standard plus program.
    StandardPlus,
    /// Thermal disinfection program (75 °C, 10 min).
    ThermalDisinfection75Deg10Min,
    /// Chemo/thermal disinfection program (40 °C, 20 min).
    ChemoThermalDisinfection40Deg20Min,
    /// Chemo/thermal disinfection program (71 °C, 25 min).
    ChemoThermalDisinfection71Deg25Min,
    /// Hygiene disinfection program (40 °C, 20 min).
    HygieneDisinfection40Deg20Min,
    /// Hygiene disinfection program (60 °C, 20 min).
    HygieneDisinfection60Deg20Min,
    /// Hygiene disinfection program (70 °C, 10 min).
    HygieneDisinfection70Deg10Min,
    /// Hygiene disinfection program (85 °C, 15 min).
    HygieneDisinfection85Deg15Min,
    /// Wash cloths program.
    WashCloths,
    /// Intensive program.
    Intensive,
    /// New mops program.
    MopsNew,
    /// Clean machine program.
    CleanMachine,
    /// Intensive plus program.
    IntensivePlus,
    /// Indoor sportswear program.
    SportswearIndoor,
    /// Outdoor sportswear program.
    SportswearOutdoor,
    /// Down comforters program.
    ComfortersDown,
    /// Synthetic comforters program.
    ComfortersSynthetic,
    /// Wool comforters program.
    ComfortersWool,
    /// Undergarment sanitization program.
    UndergarmentSanitization,
    /// Workwear program.
    Workwear,
    /// Workwear plus program.
    WorkwearPlus,
    /// Mops program.
    Mops,
    /// Mops chemo/thermal program (10 min).
    MopsChemoThermal10Min,
    /// Mops chemo/thermal program (20 min).
    MopsChemoThermal20Min,
    /// Mops chemo/thermal program (30 min).
    MopsChemoThermal30Min,
    /// Mops thermal program (15 min).
    MopsThermal15Min,
    /// Finish cloths program.
    ClothsFinish,
    /// Wash/finish cloths program.
    ClothsWashFinish,
    /// Greasy cloths program.
    ClothsGreasy,
    /// Masks program (10 min).
    Masks10Min,
    /// Masks program (20 min).
    Masks20Min,
    /// Masks program (30 min).
    Masks30Min,
    /// Protective clothing program.
    ProtectiveClothingWash,
    /// Protective clothing wash/proof program.
    ProtectiveClothingWashProof,
    /// Protective clothing proof program.
    ProtectiveClothingProof,
    /// Sensitive wetcare program.
    WetcareSensitive,
    /// Silks wetcare program.
    WetcareSilks,
    /// Intensive wetcare program.
    WetcareIntensive,
    /// Horse blankets program.
    HorseBlankets,
    /// Wool horse blankets program.
    HorseBlanketsWool,
    /// Special program 4.
    SpecialProgram4,
    /// Special program 5.
    SpecialProgram5,
    /// Express 20 program.
    Express20,
    /// Darks/denim program.
    DarksDenim,
    /// Kids quick wash program.
    KidsQuickWash,
    /// Sensitive baby program.
    BabySensitive,
    /// Quick wash baby program.
    BabyQuickWash,
    /// Sensitive towelling baby program.
    BabySensitiveTowelling,
    /// Bathrobes program.
    Bathrobes,
    /// Down filled items program.
    DownFilledItems,
    /// Toy building blocks program.
    ToyBuildingBlocks,
    /// Lingerie program.
    Lingerie,
    /// Mops finish program.
    MopsFinish,
    /// Cottons eco program.
    CottonsEco,
    /// Cottons eco plus program.
    CottonsEcoPlus,
    /// Minimum iron eco program.
    MinimumIronEco,
    /// Hard parts program.
    HardParts,
    /// Delicate curtains program.
    CurtainsDelicate,
    /// Mops wash/finish program (4 drops).
    MopsWashFinish4Drops,
    /// Mops wash/finish program (3 drops).
    MopsWashFinish3Drops,
    /// Mops wash/finish program (2 drops).
    MopsWashFinish2Drops,
    /// Mops wash/finish program (1 drop).
    MopsWashFinish1Drop,
    /// Mops finish program (4 drops).
    MopsFinish4Drops,
    /// Mops finish program (3 drops).
    MopsFinish3Drops,
    /// Mops finish program (2 drops).
    MopsFinish2Drops,
    /// Mops finish program (1 drop).
    MopsFinish1Drop,
    /// Quick power wash program.
    QuickPowerWash,
    /// Woolens/silks program.
    WoolensSilks,
    /// Refresh/dry program.
    RefreshDry,
    /// Steam finish program.
    FinishSteam,
    /// Quick power wash program (washer-dryer).
    QuickPowerWashDry,
    /// Express wash program (washer-dryer).
    ExpressWashDry,
    /// First use program.
    FirstUse,
}

/// Shared fault definitions for different device types.
pub mod faults {
    use crate::device::{Property, PropertyKind};

    pub(crate) const PROP_FAULT_F1: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f1",
        name: "F1: NTC Thermistor Short (Water Path)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F2: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f2",
        name: "F2: NTC Thermistor Open (Water Path)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F3: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f3",
        name: "F3: NTC Thermistor Short (Air Path)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F4: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f4",
        name: "F4: NTC Thermistor Open (Air Path)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F10: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f10",
        name: "F10: Cold Water Inlet",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F11: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f11",
        name: "F11: Drainage",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F12: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f12",
        name: "F12: Water Inlet Start",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F13: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f13",
        name: "F13: Water Inlet End",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F15: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f15",
        name: "F15: Hot Water Inlet",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F16: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f16",
        name: "F16: Detergent Overdose",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F19: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f19",
        name: "F19: Flow Meter",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F20: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f20",
        name: "F20: Heater",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F34: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f34",
        name: "F34: Door Locking",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F35: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f35",
        name: "F35: Door Unlocking",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F39: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f39",
        name: "F39: Control Electronics",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F41: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f41",
        name: "F41: EEPROM",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F43: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f43",
        name: "F43: Device Type",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F45: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f45",
        name: "F45: Flash/RAM",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F46: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f46",
        name: "F46: Display",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F47: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f47",
        name: "F47: Board Interface",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F49: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f49",
        name: "F49: Auxiliary Relay Board",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F50: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f50",
        name: "F50: Drum Motor",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F51: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f51",
        name: "F51: Pressure Sensor",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F53: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f53",
        name: "F53: Tachometer",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F55: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f55",
        name: "F55: Timeout (Dryer)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F56: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f56",
        name: "F56: Final Spin Speed",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F62: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f62",
        name: "F62: Program Selector",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F63: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f63",
        name: "F63: Water Diverter",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F64: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f64",
        name: "F64: Load Sensor",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F65: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f65",
        name: "F65: Drum Light Cap",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F81: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f81",
        name: "F81: Steam Inactive",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F83: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f83",
        name: "F83: Excessive Steam Temperature",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F92: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f92",
        name: "F92: Hygiene Info",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F93: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f93",
        name: "F93: Auxiliary Relay Board",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F96: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f96",
        name: "F96: Gray Water Inlet",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F100: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f100",
        name: "F100: IK6 Communication",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F101: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f101",
        name: "F101: IK6 Defective/Incompatible",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F102: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f102",
        name: "F102: Smart Home Communication",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F103: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f103",
        name: "F103: Smart Home Incompatible",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F104: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f104",
        name: "F104: Drum Motor Low Voltage",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F105: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f105",
        name: "F105: NTC Thermistor Short (Steam Path)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F106: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f106",
        name: "F106: NTC Thermistor Open (Steam Path)",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F130: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f130",
        name: "F130: EZL Communication",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F131: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f131",
        name: "F131: EZL Defective/Incompatible",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F138: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f138",
        name: "F138: Drip Tray Water",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F139: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f139",
        name: "F139: WPS Dispenser",
        unit: None,
    };
    pub(crate) const PROP_FAULT_F140: Property = Property {
        kind: PropertyKind::Fault,
        id: "fault_f140",
        name: "F140: Drainage Dispenser",
        unit: None,
    };

    /// Standardized fault code.
    ///
    /// Each code represents a specific fault condition that can occur in a machine.
    ///
    /// This enum is marked `#[non_exhaustive]` to allow for future variants.
    #[non_exhaustive]
    #[derive(PartialEq, Eq, Copy, Clone, Debug)]
    #[repr(u16)]
    pub enum FaultCode {
        /// NTC thermistor (temperature sensor) short circuit fault (water path).
        NtcThermistorShortWater = 1,
        /// NTC thermistor (temperature sensor) open circuit fault (water path).
        NtcThermistorOpenWater = 2,
        /// NTC thermistor (temperature sensor) short circuit fault (air path).
        NtcThermistorShortAir = 3,
        /// NTC thermistor (temperature sensor) open circuit fault (air path).
        NtcThermistorOpenAir = 4,
        /// Cold water inlet fault.
        ColdWaterInlet = 10,
        /// Drainage fault.
        Drainage = 11,
        /// Water inlet fault at start of step.
        WaterInletStart = 12,
        /// Water inlet fault at end of step.
        WaterInletEnd = 13,
        /// Hot water inlet fault.
        HotWaterInlet = 15,
        /// Detergent overdose fault.
        DetergentOverdose = 16,
        /// Flow meter fault.
        FlowMeter = 19,
        /// Heater fault.
        Heater = 20,
        /// Door locking fault.
        DoorLocking = 34,
        /// Door unlocking fault.
        DoorUnlocking = 35,
        /// Control electronics fault.
        ControlElectronics = 39,
        /// EEPROM fault.
        Eeprom = 41,
        /// Device type fault.
        DeviceType = 43,
        /// Flash/RAM fault.
        FlashRam = 45,
        /// Display fault.
        Display = 46,
        /// Board interface fault.
        BoardInterface = 47,
        /// Auxiliary relay board fault.
        AuxiliaryRelayBoard = 49,
        /// Drum motor fault.
        DrumMotor = 50,
        /// Pressure sensor fault.
        PressureSensor = 51,
        /// Tachometer fault.
        Tachometer = 53,
        /// Timeout fault (dryer).
        TimeoutDryer = 55,
        /// Final spin cycle speed too low (< 400 rpm) fault.
        FinalSpinSpeed = 56,
        /// Program selector fault.
        ProgramSelector = 62,
        /// Water diverter fault.
        WaterDiverter = 63,
        /// Load sensor fault.
        LoadSensor = 64,
        /// Drum light cap fault.
        DrumLightCap = 65,
        /// Steam inactive fault.
        SteamInactive = 81,
        /// Steam excessive temperature fault.
        SteamExcessiveTemperature = 83,
        /// Hygiene info fault.
        HygieneInfo = 92,
        /// Auxiliary relay board fault (alternative).
        AuxiliaryRelayBoardAlt = 93,
        /// Gray water inlet fault.
        GrayWaterInlet = 96,
        /// IK6 communication fault.
        Ik6Communication = 100,
        /// IK6 defective/incompatible fault.
        Ik6DefectiveIncompatible = 101,
        /// Smart home communication fault.
        SmartHomeCommunication = 102,
        /// Smart home incompatible fault.
        SmartHomeIncompatible = 103,
        /// Drum motor low voltage fault.
        DrumMotorLowVoltage = 104,
        /// NTC thermistor (temperature sensor) short circuit fault (steam path).
        NtcThermistorShortSteam = 105,
        /// NTC thermistor (temperature sensor) open circuit fault (steam path).
        NtcThermistorOpenSteam = 106,
        /// EZL communication fault.
        EzlCommunication = 130,
        /// EZL defective/incompatible fault.
        EzlDefectiveIncompatible = 131,
        /// Drip tray water fault.
        DripTrayWater = 138,
        /// WPS dispenser fault.
        WpsDispenser = 139,
        /// Drainage dispenser fault.
        DrainageDispenser = 140,
    }

    pub(crate) fn prop_to_fault_code(prop: &Property) -> Option<FaultCode> {
        match *prop {
            PROP_FAULT_F1 => Some(FaultCode::NtcThermistorShortWater),
            PROP_FAULT_F2 => Some(FaultCode::NtcThermistorOpenWater),
            PROP_FAULT_F3 => Some(FaultCode::NtcThermistorShortAir),
            PROP_FAULT_F4 => Some(FaultCode::NtcThermistorOpenAir),
            PROP_FAULT_F10 => Some(FaultCode::ColdWaterInlet),
            PROP_FAULT_F11 => Some(FaultCode::Drainage),
            PROP_FAULT_F12 => Some(FaultCode::WaterInletStart),
            PROP_FAULT_F13 => Some(FaultCode::WaterInletEnd),
            PROP_FAULT_F15 => Some(FaultCode::HotWaterInlet),
            PROP_FAULT_F16 => Some(FaultCode::DetergentOverdose),
            PROP_FAULT_F19 => Some(FaultCode::FlowMeter),
            PROP_FAULT_F20 => Some(FaultCode::Heater),
            PROP_FAULT_F34 => Some(FaultCode::DoorLocking),
            PROP_FAULT_F35 => Some(FaultCode::DoorUnlocking),
            PROP_FAULT_F39 => Some(FaultCode::ControlElectronics),
            PROP_FAULT_F41 => Some(FaultCode::Eeprom),
            PROP_FAULT_F43 => Some(FaultCode::DeviceType),
            PROP_FAULT_F45 => Some(FaultCode::FlashRam),
            PROP_FAULT_F46 => Some(FaultCode::Display),
            PROP_FAULT_F47 => Some(FaultCode::BoardInterface),
            PROP_FAULT_F49 => Some(FaultCode::AuxiliaryRelayBoard),
            PROP_FAULT_F50 => Some(FaultCode::DrumMotor),
            PROP_FAULT_F51 => Some(FaultCode::PressureSensor),
            PROP_FAULT_F53 => Some(FaultCode::Tachometer),
            PROP_FAULT_F55 => Some(FaultCode::TimeoutDryer),
            PROP_FAULT_F56 => Some(FaultCode::FinalSpinSpeed),
            PROP_FAULT_F62 => Some(FaultCode::ProgramSelector),
            PROP_FAULT_F63 => Some(FaultCode::WaterDiverter),
            PROP_FAULT_F64 => Some(FaultCode::LoadSensor),
            PROP_FAULT_F65 => Some(FaultCode::DrumLightCap),
            PROP_FAULT_F81 => Some(FaultCode::SteamInactive),
            PROP_FAULT_F83 => Some(FaultCode::SteamExcessiveTemperature),
            PROP_FAULT_F92 => Some(FaultCode::HygieneInfo),
            PROP_FAULT_F93 => Some(FaultCode::AuxiliaryRelayBoardAlt),
            PROP_FAULT_F96 => Some(FaultCode::GrayWaterInlet),
            PROP_FAULT_F100 => Some(FaultCode::Ik6Communication),
            PROP_FAULT_F101 => Some(FaultCode::Ik6DefectiveIncompatible),
            PROP_FAULT_F102 => Some(FaultCode::SmartHomeCommunication),
            PROP_FAULT_F103 => Some(FaultCode::SmartHomeIncompatible),
            PROP_FAULT_F104 => Some(FaultCode::DrumMotorLowVoltage),
            PROP_FAULT_F105 => Some(FaultCode::NtcThermistorShortSteam),
            PROP_FAULT_F106 => Some(FaultCode::NtcThermistorOpenSteam),
            PROP_FAULT_F130 => Some(FaultCode::EzlCommunication),
            PROP_FAULT_F131 => Some(FaultCode::EzlDefectiveIncompatible),
            PROP_FAULT_F138 => Some(FaultCode::DripTrayWater),
            PROP_FAULT_F139 => Some(FaultCode::WpsDispenser),
            PROP_FAULT_F140 => Some(FaultCode::DrainageDispenser),
            _ => None,
        }
    }
}
