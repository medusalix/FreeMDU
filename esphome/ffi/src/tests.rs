//! Runs the C interface against an emulated machine.
//!
//! The emulator answers the diagnostic protocol from real memory dumps of a Miele W 471
//! (software ID 457, Cottons 60 selected, door closed), so a test passes through the actual
//! `id457` device implementation, byte by byte, including the optical echo.

use super::*;
use std::{collections::VecDeque, ffi::CStr};

const SOFTWARE_ID: u16 = 457;
const READ_KEY: u16 = 0x43ea;
static RAM: &[u8; 1024] = include_bytes!("fixtures/id457-ram-cottons60.bin");
static EEPROM: &[u8; 2048] = include_bytes!("fixtures/id457-eeprom.bin");

enum State {
    /// Collecting a request: four bytes and a checksum.
    Request(Vec<u8>),
    /// Sending a response payload; waiting for the acknowledgement of the last chunk.
    Response(VecDeque<u8>),
}

struct Machine {
    ram: [u8; 1024],
    state: State,
    output: VecDeque<u8>,
    echo: bool,
    unlocked: bool,
    answering: bool,
}

impl Machine {
    fn new(echo: bool) -> Self {
        Self {
            ram: *RAM,
            state: State::Request(Vec::new()),
            output: VecDeque::new(),
            echo,
            unlocked: false,
            answering: true,
        }
    }

    fn send_chunk(&mut self, payload: &mut VecDeque<u8>) {
        let chunk: Vec<u8> = payload.drain(..payload.len().min(4)).collect();
        let checksum = chunk.iter().fold(0u8, |sum, &x| sum.wrapping_add(x));

        self.output.extend(chunk);
        self.output.push_back(checksum);
    }

    fn respond(&mut self, payload: Vec<u8>) {
        if payload.is_empty() {
            self.state = State::Request(Vec::new());
            return;
        }

        let mut payload = VecDeque::from(payload);

        self.send_chunk(&mut payload);
        self.state = State::Response(payload);
    }

    fn handle(&mut self, req: &[u8]) {
        let checksum = req[..4].iter().fold(0u8, |sum, &x| sum.wrapping_add(x));

        if checksum != req[4] {
            self.output.push_back(0x01);
            return;
        }

        let cmd = req[0];
        let param = u16::from_le_bytes([req[1], req[2]]) as usize;
        let len = req[3] as usize;

        let payload = match cmd {
            0x11 => SOFTWARE_ID.to_le_bytes().to_vec(),
            0x20 => {
                self.unlocked = param == usize::from(READ_KEY);
                Vec::new()
            }
            0x30 if self.unlocked => self.ram[param..param + len].to_vec(),
            0x31 if self.unlocked => EEPROM[param * 2..param * 2 + len].to_vec(),
            _ => {
                self.output.push_back(0x02);
                return;
            }
        };

        self.output.push_back(0x00);
        self.respond(payload);
    }

    fn write(&mut self, data: &[u8]) {
        if self.echo {
            self.output.extend(data);
        }

        if !self.answering {
            return;
        }

        for &byte in data {
            match &mut self.state {
                State::Request(buf) => {
                    buf.push(byte);

                    if buf.len() == 5 {
                        let req = core::mem::take(buf);

                        self.handle(&req);
                    }
                }
                State::Response(payload) => {
                    assert_eq!(byte, 0x00, "host must acknowledge every chunk");

                    let mut payload = core::mem::take(payload);

                    if payload.is_empty() {
                        self.state = State::Request(Vec::new());
                    } else {
                        self.send_chunk(&mut payload);
                        self.state = State::Response(payload);
                    }
                }
            }
        }
    }
}

unsafe extern "C" fn read(ctx: *mut c_void, buf: *mut u8, len: usize, _timeout_ms: u32) -> i32 {
    let machine = unsafe { &mut *ctx.cast::<Machine>() };
    let buf = unsafe { slice::from_raw_parts_mut(buf, len) };
    let mut n = 0;

    while n < len
        && let Some(byte) = machine.output.pop_front()
    {
        buf[n] = byte;
        n += 1;
    }

    n as i32
}

unsafe extern "C" fn write(ctx: *mut c_void, buf: *const u8, len: usize) -> i32 {
    let machine = unsafe { &mut *ctx.cast::<Machine>() };

    machine.write(unsafe { slice::from_raw_parts(buf, len) });

    len as i32
}

fn port(machine: &mut Machine, discard_echo: bool) -> fmdu_port_t {
    fmdu_port_t {
        ctx: ptr::from_mut(machine).cast(),
        read: Some(read),
        write: Some(write),
        timeout_ms: 100,
        discard_echo,
    }
}

fn connect(machine: &mut Machine, discard_echo: bool) -> (i32, *mut fmdu_device_t, u16) {
    let port = port(machine, discard_echo);
    let mut dev = ptr::null_mut();
    let mut id = 0;
    let status = unsafe { fmdu_connect(&port, &mut dev, &mut id) };

    (status, dev, id)
}

fn query(dev: *mut fmdu_device_t, id: &str) -> fmdu_value_t {
    let idx = unsafe { fmdu_property_find(dev, id.as_ptr().cast(), id.len()) };
    assert!(idx >= 0, "property {id} should exist");

    let mut val = fmdu_value_t {
        kind: 0xff,
        number: 0,
        target: 0,
        count: 0,
        text: [0; FMDU_TEXT_MAX],
        name: [0x7f; FMDU_TEXT_MAX],
    };
    let status = unsafe { fmdu_query_property(dev, idx as usize, &mut val) };
    assert_eq!(status, FMDU_OK, "querying {id} should succeed");

    val
}

fn name(val: &fmdu_value_t) -> &str {
    unsafe { CStr::from_ptr(val.name.as_ptr()) }
        .to_str()
        .unwrap()
}

fn text(val: &fmdu_value_t) -> &str {
    unsafe { CStr::from_ptr(val.text.as_ptr()) }
        .to_str()
        .unwrap()
}

#[test]
fn reads_id457_through_the_optical_echo() {
    let mut machine = Machine::new(true);
    let (status, dev, id) = connect(&mut machine, true);

    assert_eq!(status, FMDU_OK);
    assert_eq!(id, SOFTWARE_ID);
    assert_eq!(unsafe { fmdu_software_id(dev) }, SOFTWARE_ID);

    let mut kind = [0; 32];
    unsafe { fmdu_device_kind(dev, kind.as_mut_ptr(), kind.len()) };
    assert_eq!(
        unsafe { CStr::from_ptr(kind.as_ptr()) }.to_str().unwrap(),
        "Washing Machine"
    );

    let state = query(dev, "operating_state");
    assert_eq!(
        (state.kind, text(&state)),
        (FMDU_VALUE_STRING, "ProgramIdle")
    );

    assert_eq!(name(&state), "Ready");

    let program = query(dev, "selected_program");
    assert_eq!(text(&program), "Cottons60");
    assert_eq!(name(&program), "Cottons 60 °C");

    let temp = query(dev, "program_temperature");
    assert_eq!(name(&temp), "", "numbers have no display name");
    assert_eq!((temp.kind, temp.number), (FMDU_VALUE_NUMBER, 60));

    let spin = query(dev, "program_spin_speed");
    assert_eq!((spin.number, text(&spin)), (1600, "1600"));

    let display = query(dev, "display_contents");
    assert_eq!(text(&display), "51");

    let remaining = query(dev, "remaining_time");
    assert_eq!(
        (remaining.kind, remaining.number, text(&remaining)),
        (FMDU_VALUE_DURATION, 51 * 60, "0h 51min")
    );

    let indicator = query(dev, "display_indicator");
    assert_eq!(
        (text(&indicator), name(&indicator)),
        ("RemainingTime", "Remaining time")
    );

    let date = query(dev, "manufacturing_date");
    assert_eq!(
        (date.kind, date.number, text(&date)),
        (FMDU_VALUE_DATE, 20_020_228, "2002-02-28")
    );

    let hours = query(dev, "operating_time");
    assert_eq!(hours.kind, FMDU_VALUE_DURATION);
    assert!(text(&hours).starts_with("17446h"), "got {}", text(&hours));

    let temps = query(dev, "temperature");
    assert_eq!(
        (temps.kind, temps.number, temps.target),
        (FMDU_VALUE_SENSOR, 19, 0)
    );

    let locked = query(dev, "door_locked");
    assert_eq!(
        (locked.kind, locked.number, text(&locked)),
        (FMDU_VALUE_BOOL, 0, "No")
    );

    let spinning = query(dev, "spinning");
    assert_eq!(
        (spinning.kind, spinning.number, text(&spinning)),
        (FMDU_VALUE_BOOL, 0, "No")
    );

    let load = query(dev, "load_level");
    assert_eq!((load.kind, load.number), (FMDU_VALUE_NUMBER, 1));

    let max_load = query(dev, "program_max_load");
    assert_eq!((max_load.kind, max_load.number), (FMDU_VALUE_NUMBER, 5000));

    let overdose = query(dev, "fault_f5");
    assert_eq!(
        (overdose.kind, overdose.number, text(&overdose)),
        (FMDU_VALUE_FAULT, 0, "Ok")
    );

    let drum = query(dev, "tachometer_speed");
    assert_eq!(
        (drum.kind, drum.number, drum.target),
        (FMDU_VALUE_SENSOR, 0, 0)
    );

    // Switched on, no program running: standby only
    let power = query(dev, "estimated_power");
    assert_eq!((power.kind, power.number), (FMDU_VALUE_NUMBER, 8));

    assert!(
        machine.output.is_empty(),
        "every byte the machine sent should have been read"
    );

    unsafe { fmdu_free(dev) };
}

#[test]
fn lists_properties_with_units() {
    let mut machine = Machine::new(false);
    let (status, dev, _) = connect(&mut machine, false);
    assert_eq!(status, FMDU_OK);

    let count = unsafe { fmdu_property_count(dev) };
    assert_eq!(count, 23);

    let idx = unsafe { fmdu_property_find(dev, c"program_spin_speed".as_ptr(), 18) };
    let mut prop = fmdu_property_t {
        id: ptr::null(),
        id_len: 0,
        name: ptr::null(),
        name_len: 0,
        unit: ptr::null(),
        unit_len: 0,
        kind: 0xff,
    };

    assert!(unsafe { fmdu_property_get(dev, idx as usize, &mut prop) });
    assert_eq!(prop.kind, FMDU_KIND_OPERATION);

    let unit = unsafe { slice::from_raw_parts(prop.unit.cast::<u8>(), prop.unit_len) };
    assert_eq!(unit, b"rpm");

    assert!(!unsafe { fmdu_property_get(dev, count, &mut prop) });
    assert_eq!(unsafe { fmdu_property_find(dev, c"nope".as_ptr(), 4) }, -1);

    unsafe { fmdu_free(dev) };
}

#[test]
fn reports_a_silent_machine_as_timeout() {
    let mut machine = Machine::new(true);
    machine.answering = false;

    let (status, dev, _) = connect(&mut machine, true);

    assert_eq!(status, FMDU_ERR_TIMEOUT);
    assert!(dev.is_null());
}

#[test]
fn reports_a_missing_echo() {
    // The adapter does not even see itself: a broken wire, or the sensor flooded by light
    let mut machine = Machine::new(false);
    machine.answering = false;

    let (status, _, _) = connect(&mut machine, true);

    assert_eq!(status, FMDU_ERR_NO_ECHO);
}

#[test]
fn reports_a_value_the_state_does_not_have_as_unavailable() {
    let mut machine = Machine::new(true);
    // The display shows the detergent dose, "40", as before a start
    machine.ram[0x009e..0x00a2].copy_from_slice(&[0x40, 0x00, 0x80, 0x73]);
    let (status, dev, _) = connect(&mut machine, true);
    assert_eq!(status, FMDU_OK);

    let idx = unsafe { fmdu_property_find(dev, c"remaining_time".as_ptr(), 14) };
    assert!(idx >= 0);
    let mut val = fmdu_value_t {
        kind: 0xff,
        number: 0,
        target: 0,
        count: 0,
        text: [0; FMDU_TEXT_MAX],
        name: [0; FMDU_TEXT_MAX],
    };
    let status = unsafe { fmdu_query_property(dev, idx as usize, &mut val) };
    assert_eq!(status, FMDU_ERR_UNAVAILABLE);

    // The device still answers the next query
    assert_eq!(text(&query(dev, "display_contents")), "40");

    unsafe { fmdu_free(dev) };
}

#[test]
fn status_strings_are_terminated() {
    for status in [
        FMDU_OK,
        FMDU_ERR_TIMEOUT,
        FMDU_ERR_UNKNOWN_DEVICE,
        FMDU_ERR_NO_ECHO,
        FMDU_ERR_UNAVAILABLE,
        -99,
    ] {
        let msg = unsafe { CStr::from_ptr(fmdu_status_str(status)) };

        assert!(!msg.to_bytes().is_empty());
        assert!(
            msg.to_bytes()[0].is_ascii_uppercase(),
            "{msg:?} should start upper case"
        );
    }
}

#[test]
fn truncates_long_text_at_a_character_boundary() {
    let mut val = fmdu_value_t {
        kind: 0,
        number: 0,
        target: 0,
        count: 0,
        text: [0x7f; FMDU_TEXT_MAX],
        name: [0; FMDU_TEXT_MAX],
    };

    fill_value(&Value::String("ä".repeat(40)), &mut val);

    let text = text(&val);
    assert_eq!(text.chars().count(), 31);
    assert_eq!(val.text[FMDU_TEXT_MAX - 1], 0);
}

#[test]
fn reads_raw_memory_in_chunks() {
    let mut machine = Machine::new(true);
    let (status, dev, _) = connect(&mut machine, true);
    assert_eq!(status, FMDU_OK);

    let mut buf = [0u8; 64];
    let status = unsafe { fmdu_read_memory(dev, 0x00c0, buf.as_mut_ptr(), buf.len()) };

    assert_eq!(status, FMDU_OK);
    assert_eq!(&buf[..], &RAM[0x00c0..0x0100]);
    assert!(machine.output.is_empty());

    let status = unsafe { fmdu_read_memory(dev, 0x00c0, buf.as_mut_ptr(), 10) };
    assert_eq!(
        status, FMDU_ERR_ARGUMENT,
        "length must be a multiple of the chunk size"
    );

    unsafe { fmdu_free(dev) };
}
