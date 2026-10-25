//! C interface to the FreeMDU protocol crate.
//!
//! Built as a static library and linked into the ESPHome component in `../components/freemdu`,
//! so that the device implementations in the `freemdu` crate are used as they are instead of
//! being re-implemented in C++.
//!
//! All I/O goes through two callbacks supplied by the caller. They block until data has been
//! transferred or a timeout has expired, so none of the protocol futures ever has to wait for a
//! waker and each call is driven to completion by [`block_on`]. The caller is expected to run
//! these functions from a task of its own, because a single poll of a device takes a second or
//! more at 2400 baud.
//!
//! The matching C declarations are in `../components/freemdu/freemdu_ffi.h`.

// Bare metal on the microcontroller; with std on the host, where the tests run
#![cfg_attr(target_os = "none", no_std)]
#![allow(non_camel_case_types)]

extern crate alloc;

use alloc::boxed::Box;
use core::{
    ffi::{c_char, c_void},
    fmt::{self, Write as _},
    pin::pin,
    ptr, slice,
    sync::atomic::{AtomicPtr, Ordering},
    task::{Context, Poll, Waker},
};
use embedded_io_async::{ErrorKind, ErrorType, Read, Write};
use freemdu::device::{self, Date, Device, Error, Fault, PropertyKind, Value};

/// Operation succeeded.
pub const FMDU_OK: i32 = 0;
/// The device did not answer in time.
pub const FMDU_ERR_TIMEOUT: i32 = -1;
/// A port callback reported an error.
pub const FMDU_ERR_PORT: i32 = -2;
/// The device answered, but its software ID is not supported.
pub const FMDU_ERR_UNKNOWN_DEVICE: i32 = -3;
/// The device sent something the protocol did not expect.
pub const FMDU_ERR_PROTOCOL: i32 = -4;
/// An argument was invalid, e.g. a property index out of range.
pub const FMDU_ERR_ARGUMENT: i32 = -5;
/// The device is in a state or reported a value the implementation does not handle.
pub const FMDU_ERR_DEVICE: i32 = -6;
/// Any other error.
pub const FMDU_ERR_OTHER: i32 = -7;
/// With `discard_echo`, the adapter's own transmission did not come back: the receiver sees
/// nothing at all. On the appliance's window, which reflects it, the fault is in the adapter
/// (wiring, or flooded by light), not in alignment.
pub const FMDU_ERR_NO_ECHO: i32 = -8;
/// The property has no value in the appliance's current state, e.g. a remaining time while the
/// display shows something else. Not a fault: [`fmdu_query_property`] logs nothing for it.
pub const FMDU_ERR_UNAVAILABLE: i32 = -9;

/// Maximum length of the text representation of a value, including the terminating NUL.
pub const FMDU_TEXT_MAX: usize = 64;

/// Reads up to `len` bytes into `buf`, waiting at most `timeout_ms` for the first byte.
/// Returns the number of bytes read, 0 on timeout or a negative number on error.
pub type fmdu_read_fn = unsafe extern "C" fn(*mut c_void, *mut u8, usize, u32) -> i32;
/// Writes `len` bytes from `buf`. Returns the number of bytes written or a negative number on error.
pub type fmdu_write_fn = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
/// Receives one log line. `level` follows the `log` crate: 1 error, 2 warn, 3 info, 4 debug, 5 trace.
pub type fmdu_log_fn = unsafe extern "C" fn(u8, *const c_char, usize);

/// Serial port supplied by the caller.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fmdu_port_t {
    /// Passed unchanged to the callbacks.
    pub ctx: *mut c_void,
    pub read: Option<fmdu_read_fn>,
    pub write: Option<fmdu_write_fn>,
    /// Timeout passed to `read`.
    pub timeout_ms: u32,
    /// Read back and discard every written byte. Needed for the optical interface,
    /// where the receiver sees the adapter's own transmitter.
    pub discard_echo: bool,
}

/// Description of a device property.
///
/// The strings are not NUL-terminated and live as long as the library.
#[repr(C)]
pub struct fmdu_property_t {
    pub id: *const c_char,
    pub id_len: usize,
    pub name: *const c_char,
    pub name_len: usize,
    /// Null if the property has no unit.
    pub unit: *const c_char,
    pub unit_len: usize,
    /// One of the `FMDU_KIND_*` constants.
    pub kind: u8,
}

pub const FMDU_KIND_GENERAL: u8 = 0;
pub const FMDU_KIND_FAULT: u8 = 1;
pub const FMDU_KIND_OPERATION: u8 = 2;
pub const FMDU_KIND_IO: u8 = 3;

pub const FMDU_VALUE_BOOL: u8 = 0;
pub const FMDU_VALUE_NUMBER: u8 = 1;
pub const FMDU_VALUE_SENSOR: u8 = 2;
pub const FMDU_VALUE_STRING: u8 = 3;
pub const FMDU_VALUE_DURATION: u8 = 4;
pub const FMDU_VALUE_DATE: u8 = 5;
pub const FMDU_VALUE_FAULT: u8 = 6;

/// A property value.
///
/// | `kind`     | `number`                       | `target`        | `count`     |
/// |------------|--------------------------------|-----------------|-------------|
/// | `BOOL`     | 0 or 1                         |                 |             |
/// | `NUMBER`   | the number                     |                 |             |
/// | `SENSOR`   | current value                  | target value    |             |
/// | `STRING`   |                                |                 |             |
/// | `DURATION` | seconds                        |                 |             |
/// | `DATE`     | `YYYYMMDD`                     |                 |             |
/// | `FAULT`    | 0 ok, 1 active, 2 stored       | operating hours | occurrences |
///
/// `text` always holds a NUL-terminated, human-readable form, truncated if necessary, and
/// `name` the device implementation's display name for it, if any.
#[repr(C)]
pub struct fmdu_value_t {
    pub kind: u8,
    pub number: u32,
    pub target: u32,
    pub count: u32,
    pub text: [c_char; FMDU_TEXT_MAX],
    /// Display name from the device implementation, e.g. "Running" for `ProgramRunning`;
    /// empty if it has none. `text` keeps the identifier.
    pub name: [c_char; FMDU_TEXT_MAX],
}

/// A connected device. Created by [`fmdu_connect`], released by [`fmdu_free`].
pub struct fmdu_device_t {
    dev: Box<dyn Device<Port>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortError {
    Timeout,
    /// Timed out reading back the adapter's own transmission.
    NoEcho,
    Failed,
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "timeout"),
            Self::NoEcho => write!(f, "no echo of the transmitted bytes"),
            Self::Failed => write!(f, "port error"),
        }
    }
}

impl core::error::Error for PortError {}

impl embedded_io_async::Error for PortError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Timeout | Self::NoEcho => ErrorKind::TimedOut,
            Self::Failed => ErrorKind::Other,
        }
    }
}

/// Adapts the caller's callbacks to the `embedded-io-async` traits used by the protocol crate.
pub struct Port(fmdu_port_t);

impl ErrorType for Port {
    type Error = PortError;
}

impl Read for Port {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, PortError> {
        if buf.is_empty() {
            return Ok(0);
        }

        let read = self.0.read.ok_or(PortError::Failed)?;
        let len = unsafe { read(self.0.ctx, buf.as_mut_ptr(), buf.len(), self.0.timeout_ms) };

        match len {
            0 => Err(PortError::Timeout),
            len if len > 0 => Ok((len as usize).min(buf.len())),
            _ => Err(PortError::Failed),
        }
    }
}

impl Write for Port {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, PortError> {
        let write = self.0.write.ok_or(PortError::Failed)?;
        let len = unsafe { write(self.0.ctx, buf.as_ptr(), buf.len()) };

        if len <= 0 {
            return Err(PortError::Failed);
        }

        let len = (len as usize).min(buf.len());

        if self.0.discard_echo {
            let mut echo = [0x00; 16];
            let mut left = len;

            while left > 0 {
                let n = left.min(echo.len());

                left -= match self.read(&mut echo[..n]).await {
                    Err(PortError::Timeout) => return Err(PortError::NoEcho),
                    result => result?,
                };
            }
        }

        Ok(len)
    }

    async fn flush(&mut self) -> Result<(), PortError> {
        Ok(())
    }
}

/// Drives a future to completion.
///
/// The port callbacks block, so the protocol futures never return `Pending` in practice;
/// should one do so anyway, it is simply polled again.
fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());

    loop {
        if let Poll::Ready(val) = fut.as_mut().poll(&mut cx) {
            return val;
        }
    }
}

fn status(err: &Error<PortError>) -> i32 {
    use freemdu::Error as ProtocolError;

    match err {
        Error::Protocol(ProtocolError::Io(PortError::Timeout)) => FMDU_ERR_TIMEOUT,
        Error::Protocol(ProtocolError::Io(PortError::NoEcho)) => FMDU_ERR_NO_ECHO,
        Error::Protocol(ProtocolError::Io(PortError::Failed)) => FMDU_ERR_PORT,
        Error::Protocol(_) => FMDU_ERR_PROTOCOL,
        Error::UnknownSoftwareId(_) => FMDU_ERR_UNKNOWN_DEVICE,
        Error::InvalidArgument | Error::UnknownProperty | Error::UnknownAction => FMDU_ERR_ARGUMENT,
        Error::InvalidState | Error::UnexpectedMemoryValue => FMDU_ERR_DEVICE,
        _ => FMDU_ERR_OTHER,
    }
}

/// Writes into a fixed C string buffer, truncating at a character boundary
/// and always leaving room for the terminating NUL.
struct Text<'a> {
    buf: &'a mut [c_char; FMDU_TEXT_MAX],
    len: usize,
}

impl<'a> Text<'a> {
    fn new(buf: &'a mut [c_char; FMDU_TEXT_MAX]) -> Self {
        buf.fill(0);

        Self { buf, len: 0 }
    }
}

impl fmt::Write for Text<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            let mut enc = [0x00; 4];
            let bytes = ch.encode_utf8(&mut enc).as_bytes();

            if self.len + bytes.len() >= FMDU_TEXT_MAX {
                return Ok(());
            }

            for &byte in bytes {
                self.buf[self.len] = byte as c_char;
                self.len += 1;
            }
        }

        Ok(())
    }
}

fn fill_value(val: &Value, out: &mut fmdu_value_t) {
    out.number = 0;
    out.target = 0;
    out.count = 0;

    let mut text = Text::new(&mut out.text);

    // Formatting into a fixed buffer cannot fail; overlong text is truncated
    let _ = match *val {
        Value::Bool(val) => {
            out.kind = FMDU_VALUE_BOOL;
            out.number = val.into();
            text.write_str(if val { "Yes" } else { "No" })
        }
        Value::Number(num) => {
            out.kind = FMDU_VALUE_NUMBER;
            out.number = num;
            write!(text, "{num}")
        }
        Value::Sensor(current, target) => {
            out.kind = FMDU_VALUE_SENSOR;
            out.number = current;
            out.target = target;
            write!(text, "{current}/{target}")
        }
        Value::String(ref string) => {
            out.kind = FMDU_VALUE_STRING;
            text.write_str(string)
        }
        Value::Duration(dur) => {
            let secs = dur.as_secs();
            let total_mins = secs / 60;

            out.kind = FMDU_VALUE_DURATION;
            out.number = secs.try_into().unwrap_or(u32::MAX);
            write!(text, "{}h {}min", total_mins / 60, total_mins % 60)
        }
        Value::Date(Date { year, month, day }) => {
            out.kind = FMDU_VALUE_DATE;
            out.number = u32::from(year) * 10_000 + u32::from(month) * 100 + u32::from(day);
            write!(text, "{year}-{month:02}-{day:02}")
        }
        Value::Fault(ref fault) => {
            out.kind = FMDU_VALUE_FAULT;

            let (state, name, info) = match fault {
                Fault::Ok => (0, "Ok", None),
                Fault::Active(info) => (1, "Active", info.as_ref()),
                Fault::Stored(info) => (2, "Stored", info.as_ref()),
            };

            out.number = state;

            if let Some(info) = info {
                out.target = info.operating_hours;
                out.count = info.count;
                write!(text, "{name} ({}x, {} h)", info.count, info.operating_hours)
            } else {
                text.write_str(name)
            }
        }
    };
}

/// Connects to the device on `port`: queries its software ID and unlocks it
/// with the matching device implementation.
///
/// On success stores the device in `*out_dev`. The software ID is stored in `*out_software_id`
/// whenever the device answered, including for [`FMDU_ERR_UNKNOWN_DEVICE`]; it may be null.
///
/// # Safety
///
/// `port` must point to a valid port whose callbacks stay usable until [`fmdu_free`],
/// and `out_dev` must be valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_connect(
    port: *const fmdu_port_t,
    out_dev: *mut *mut fmdu_device_t,
    out_software_id: *mut u16,
) -> i32 {
    if port.is_null() || out_dev.is_null() {
        return FMDU_ERR_ARGUMENT;
    }

    let port = Port(unsafe { *port });

    match block_on(device::connect(port)) {
        Ok(dev) => {
            if !out_software_id.is_null() {
                unsafe { *out_software_id = dev.software_id() };
            }

            unsafe { *out_dev = Box::into_raw(Box::new(fmdu_device_t { dev })) };

            FMDU_OK
        }
        Err(err) => {
            if let Error::UnknownSoftwareId(id) = err
                && !out_software_id.is_null()
            {
                unsafe { *out_software_id = id };
            }

            log::warn!("Failed to connect to device: {err}");

            status(&err)
        }
    }
}

/// Releases a device returned by [`fmdu_connect`]. Accepts null.
///
/// # Safety
///
/// `dev` must be null or a device returned by [`fmdu_connect`] that has not been released yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_free(dev: *mut fmdu_device_t) {
    if !dev.is_null() {
        drop(unsafe { Box::from_raw(dev) });
    }
}

/// Returns the device's software ID.
///
/// # Safety
///
/// `dev` must be a valid device.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_software_id(dev: *const fmdu_device_t) -> u16 {
    unsafe { (*dev).dev.software_id() }
}

/// Writes the device kind, e.g. `Washing Machine`, as a NUL-terminated string into `buf`.
///
/// # Safety
///
/// `dev` must be a valid device and `buf` must be valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_device_kind(dev: *const fmdu_device_t, buf: *mut c_char, len: usize) {
    if buf.is_null() || len == 0 {
        return;
    }

    let mut text = [0; FMDU_TEXT_MAX];
    let _ = write!(Text::new(&mut text), "{}", unsafe { (*dev).dev.kind() });
    let n = text.iter().position(|&c| c == 0).unwrap_or(0).min(len - 1);

    unsafe {
        ptr::copy_nonoverlapping(text.as_ptr(), buf, n);
        *buf.add(n) = 0;
    }
}

/// Returns the number of properties the device implementation offers.
///
/// # Safety
///
/// `dev` must be a valid device.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_property_count(dev: *const fmdu_device_t) -> usize {
    unsafe { (*dev).dev.properties().len() }
}

/// Describes property `idx`. Returns false if `idx` is out of range.
///
/// # Safety
///
/// `dev` must be a valid device and `out` must be valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_property_get(
    dev: *const fmdu_device_t,
    idx: usize,
    out: *mut fmdu_property_t,
) -> bool {
    let Some(prop) = (unsafe { (*dev).dev.properties() }).get(idx) else {
        return false;
    };

    let (unit, unit_len) = prop
        .unit
        .map_or((ptr::null(), 0), |unit| (unit.as_ptr().cast(), unit.len()));

    unsafe {
        *out = fmdu_property_t {
            id: prop.id.as_ptr().cast(),
            id_len: prop.id.len(),
            name: prop.name.as_ptr().cast(),
            name_len: prop.name.len(),
            unit,
            unit_len,
            kind: match prop.kind {
                PropertyKind::General => FMDU_KIND_GENERAL,
                PropertyKind::Fault => FMDU_KIND_FAULT,
                PropertyKind::Operation => FMDU_KIND_OPERATION,
                PropertyKind::Io => FMDU_KIND_IO,
            },
        };
    }

    true
}

/// Returns the index of the property with the given ID, or -1 if there is none.
///
/// # Safety
///
/// `dev` must be a valid device and `id` must be valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_property_find(
    dev: *const fmdu_device_t,
    id: *const c_char,
    len: usize,
) -> i32 {
    if id.is_null() {
        return -1;
    }

    let id = unsafe { slice::from_raw_parts(id.cast::<u8>(), len) };

    (unsafe { (*dev).dev.properties() })
        .iter()
        .position(|prop| prop.id.as_bytes() == id)
        .map_or(-1, |idx| idx as i32)
}

/// Queries property `idx` from the device and stores its value in `*out`.
///
/// Returns [`FMDU_ERR_UNAVAILABLE`] when the property has no value in the current state.
///
/// # Safety
///
/// `dev` must be a valid device and `out` must be valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_query_property(
    dev: *mut fmdu_device_t,
    idx: usize,
    out: *mut fmdu_value_t,
) -> i32 {
    let dev = unsafe { &mut (*dev).dev };
    let Some(prop) = dev.properties().get(idx) else {
        return FMDU_ERR_ARGUMENT;
    };

    match block_on(dev.query_property(prop)) {
        Ok(val) => {
            let out = unsafe { &mut *out };

            fill_value(&val, out);

            let mut name = Text::new(&mut out.name);
            if let Some(display) = dev.value_name(prop, &val) {
                let _ = name.write_str(display);
            }

            FMDU_OK
        }
        // Device implementations report a value that does not exist right now this way
        Err(Error::InvalidState) => FMDU_ERR_UNAVAILABLE,
        Err(err) => {
            log::warn!("Failed to query property {}: {err}", prop.id);

            status(&err)
        }
    }
}

/// Size of each request [`fmdu_read_memory`] makes; `len` must be a multiple of it.
pub const FMDU_MEMORY_CHUNK: usize = 16;

fn protocol_status(err: &freemdu::Error<PortError>) -> i32 {
    match err {
        freemdu::Error::Io(PortError::Timeout) => FMDU_ERR_TIMEOUT,
        freemdu::Error::Io(PortError::Failed) => FMDU_ERR_PORT,
        freemdu::Error::InvalidArgument => FMDU_ERR_ARGUMENT,
        _ => FMDU_ERR_PROTOCOL,
    }
}

/// Reads `len` bytes of the device's memory from `addr` into `buf`, in requests of
/// [`FMDU_MEMORY_CHUNK`] bytes, using the access level the device implementation unlocked.
///
/// Meant for mapping a device's memory, e.g. logging it while the appliance runs.
///
/// # Safety
///
/// `dev` must be a valid device and `buf` must be valid for writes of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_read_memory(
    dev: *mut fmdu_device_t,
    addr: u32,
    buf: *mut u8,
    len: usize,
) -> i32 {
    if buf.is_null() || len % FMDU_MEMORY_CHUNK != 0 {
        return FMDU_ERR_ARGUMENT;
    }

    let intf = unsafe { (*dev).dev.interface() };
    let out = unsafe { slice::from_raw_parts_mut(buf, len) };

    for (i, chunk) in out.chunks_exact_mut(FMDU_MEMORY_CHUNK).enumerate() {
        let Ok(offset) = u32::try_from(i * FMDU_MEMORY_CHUNK) else {
            return FMDU_ERR_ARGUMENT;
        };
        let res: Result<[u8; FMDU_MEMORY_CHUNK], _> =
            block_on(intf.read_memory(addr.wrapping_add(offset)));

        match res {
            Ok(data) => chunk.copy_from_slice(&data),
            Err(err) => {
                log::warn!(
                    "Failed to read memory at {:#06x}: {err}",
                    addr.wrapping_add(offset)
                );

                return protocol_status(&err);
            }
        }
    }

    FMDU_OK
}

/// Returns a static, NUL-terminated description of a status code.
#[unsafe(no_mangle)]
pub extern "C" fn fmdu_status_str(status: i32) -> *const c_char {
    let msg: &'static [u8] = match status {
        FMDU_OK => b"Ok\0",
        FMDU_ERR_TIMEOUT => b"No answer from the device\0",
        FMDU_ERR_PORT => b"Serial port error\0",
        FMDU_ERR_UNKNOWN_DEVICE => b"Unsupported software ID\0",
        FMDU_ERR_PROTOCOL => b"Protocol error\0",
        FMDU_ERR_ARGUMENT => b"Invalid argument\0",
        FMDU_ERR_DEVICE => b"Unexpected device state or value\0",
        FMDU_ERR_NO_ECHO => b"No echo from the sensor\0",
        FMDU_ERR_UNAVAILABLE => b"No value in the current state\0",
        _ => b"Error\0",
    };

    msg.as_ptr().cast()
}

static LOG_FN: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        !LOG_FN.load(Ordering::Relaxed).is_null()
    }

    fn log(&self, record: &log::Record) {
        let func = LOG_FN.load(Ordering::Relaxed);

        if func.is_null() {
            return;
        }

        let func: fmdu_log_fn = unsafe { core::mem::transmute(func) };
        let msg = alloc::format!("{}", record.args());

        unsafe { func(record.level() as u8, msg.as_ptr().cast(), msg.len()) };
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

/// Routes the library's log output, including the protocol crate's, to `func`.
/// `max_level` follows the `log` crate (0 off … 5 trace).
///
/// # Safety
///
/// Must not be called concurrently with any other function of this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fmdu_set_logger(func: Option<fmdu_log_fn>, max_level: u8) {
    LOG_FN.store(
        func.map_or(ptr::null_mut(), |func| func as *mut c_void),
        Ordering::Relaxed,
    );

    let level = match max_level {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    unsafe {
        // Fails harmlessly if a logger has been set before
        let _ = log::set_logger_racy(&LOGGER);
        log::set_max_level_racy(level);
    }
}

#[cfg(target_os = "none")]
mod runtime {
    //! Allocator and panic handler for the bare-metal build, backed by the C library
    //! of the firmware the static library is linked into.

    use core::{alloc::GlobalAlloc, alloc::Layout, ffi::c_void, mem::size_of};

    unsafe extern "C" {
        fn malloc(size: usize) -> *mut c_void;
        fn free(ptr: *mut c_void);
        fn abort() -> !;
    }

    /// Alignment `malloc` guarantees on the 32-bit targets this is built for.
    const MALLOC_ALIGN: usize = 4;

    struct CAllocator;

    unsafe impl GlobalAlloc for CAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let size = layout.size().max(1);

            if layout.align() <= MALLOC_ALIGN {
                return unsafe { malloc(size) }.cast();
            }

            // Over-allocate and keep the original pointer just below the aligned block
            let raw = unsafe { malloc(size + layout.align() + size_of::<usize>()) };

            if raw.is_null() {
                return core::ptr::null_mut();
            }

            let addr =
                (raw as usize + size_of::<usize>() + layout.align() - 1) & !(layout.align() - 1);

            unsafe { *(addr as *mut usize).sub(1) = raw as usize };

            addr as *mut u8
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            if layout.align() <= MALLOC_ALIGN {
                unsafe { free(ptr.cast()) };
            } else {
                unsafe { free(*(ptr as *mut usize).sub(1) as *mut c_void) };
            }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CAllocator = CAllocator;

    #[panic_handler]
    fn panic(info: &core::panic::PanicInfo) -> ! {
        log::error!("{info}");

        unsafe { abort() }
    }
}

#[cfg(test)]
mod tests;
