//! Utility functions for device implementations.

/// Decodes a BCD-encoded value into a base-10 integer.
pub fn decode_bcd_value(mut val: u32) -> u32 {
    let mut mul = 1;
    let mut res = 0;

    while val > 0 {
        let digit = val & 0x0f;

        if digit <= 9 {
            res += digit * mul;
        }

        mul *= 10;
        val >>= 4;
    }

    res
}

/// Computes the resistance of an NTC thermistor from an ADC reading.
///
/// The NTC is typically connected to an ADC input according to the following schematic:
///
///    5 V
///     |
/// [2.15 kΩ]
///     |
///     |-----[RC LPF]-----> ADC Input
///     |
///   [NTC]
///     |
///    GND
pub fn ntc_resistance_from_adc(val: u8) -> u32 {
    (2150 * u32::from(val)) / (256 - u32::from(val))
}

/// Decodes raw data for a three-digit Motorola MC14489 seven-segment display into characters.
///
/// Each digit (including decimal points) is decoded using [`decode_mc14489_digit`].
pub fn decode_mc14489_display(data: [u8; 4]) -> [Option<char>; 6] {
    let points = (data[2] & 0x70) >> 4;
    let d1_code = data[0] & 0x0f;
    let d2_code = (data[0] & 0xf0) >> 4;
    let d3_code = data[1] & 0x0f;
    let d1_special = (data[3] & 0x02) != 0x00;
    let d2_special = (data[3] & 0x04) != 0x00;
    let d3_special = (data[3] & 0x08) != 0x00;
    let d1_point = points == 0x01 || points == 0x07;
    let d2_point = points == 0x02 || points == 0x07;
    let d3_point = points == 0x03 || points == 0x07;

    [
        decode_mc14489_digit(d1_code, d1_special),
        if d1_point { Some('.') } else { None },
        decode_mc14489_digit(d2_code, d2_special),
        if d2_point { Some('.') } else { None },
        decode_mc14489_digit(d3_code, d3_special),
        if d3_point { Some('.') } else { None },
    ]
}

/// Decodes a single MC14489 seven-segment digit into a character.
fn decode_mc14489_digit(code: u8, special: bool) -> Option<char> {
    match (code, special) {
        (0x00, false) => Some('0'),
        (0x01, false) => Some('1'),
        (0x02, false) => Some('2'),
        (0x03, false) => Some('3'),
        (0x04, false) => Some('4'),
        (0x05, false) => Some('5'),
        (0x06, false) => Some('6'),
        (0x07, false) => Some('7'),
        (0x08, false) => Some('8'),
        (0x09, false) => Some('9'),
        (0x0a, false) => Some('A'),
        (0x0b, false) => Some('b'),
        (0x0c, false) => Some('C'),
        (0x0d, false) => Some('d'),
        (0x0e, false) => Some('E'),
        (0x0f, false) => Some('F'),
        (0x01, true) => Some('c'),
        (0x02, true) => Some('H'),
        (0x03, true) => Some('h'),
        (0x04, true) => Some('J'),
        (0x05, true) => Some('L'),
        (0x06, true) => Some('n'),
        (0x07, true) => Some('o'),
        (0x08, true) => Some('P'),
        (0x09, true) => Some('r'),
        (0x0a, true) => Some('U'),
        (0x0b, true) => Some('u'),
        (0x0c, true) => Some('y'),
        (0x0d, true) => Some('-'),
        (0x0e, true) => Some('='),
        (0x0f, true) => Some('°'),
        _ => None,
    }
}

/// Computes the motor speed in rpm from a raw motor speed value.
pub fn rpm_from_motor_speed(speed: u32) -> Option<u16> {
    // This constant can be found by minimizing the error between the values
    // in the device's motor speed lookup table and the actual speed in rpm.
    const RPM_CONVERSION: u32 = 442_500;

    match speed {
        0x0000_0000 | 0x0000_ffff => Some(0), // No speed set
        s => (RPM_CONVERSION / s).try_into().ok(),
    }
}

/// Computes the motor speed in rpm from a raw variable-frequency drive (VFD) speed value.
pub fn rpm_from_motor_speed_vfd(speed: u16) -> u16 {
    // The VFD value and motor speed in rpm have a linear relationship.
    const RPM_CONVERSION: u16 = 113;

    match speed {
        0x7fff => 0, // No speed set
        s => (s * 10) / RPM_CONVERSION,
    }
}
