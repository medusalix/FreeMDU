#![no_std]
#![no_main]

use core::panic::PanicInfo;
use embassy_executor::Spawner;
use embassy_futures::select::{self, Either};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{self, Duration, TimeoutError};
use embedded_io_async::{Read, Write};
use esp_alloc as _;
use esp_hal::{
    Async, Config, interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup,
    usb_serial_jtag::UsbSerialJtag,
};
use freemdu_home::{OpticalPort, status_led::StatusLed};

// Buffer size for USB serial and optical port reads
const BUF_SIZE: usize = 32;

// Minimum LED on-time after each transmission
const LED_MIN_ON_DURATION: Duration = Duration::from_millis(10);

static LED_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

esp_bootloader_esp_idf::esp_app_desc!();

// Don't print backtrace on panic
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[embassy_executor::task]
async fn led_task(mut led: StatusLed<'static>) -> ! {
    loop {
        match embassy_time::with_timeout(LED_MIN_ON_DURATION, LED_SIGNAL.wait()).await {
            Ok(()) => led.on(),
            Err(TimeoutError) => led.off(),
        }
    }
}

#[embassy_executor::task]
async fn bridge_task(
    mut serial: UsbSerialJtag<'static, Async>,
    mut opt: OpticalPort<'static>,
) -> ! {
    let mut serial_buf = [0x00; BUF_SIZE];
    let mut opt_buf = [0x00; BUF_SIZE];

    loop {
        match select::select(serial.read(&mut serial_buf), opt.read(&mut opt_buf)).await {
            // Ignore all errors
            Either::First(Ok(len)) => {
                let _ = opt.write(&serial_buf[..len]).await;

                LED_SIGNAL.signal(());
            }
            Either::Second(Ok(len)) => {
                let _ = Write::write(&mut serial, &opt_buf[..len]).await;
            }
            _ => {}
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(Config::default());
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let led = freemdu_home::new_status_led(
        #[cfg(feature = "ws2812led")]
        peripherals.RMT,
    );
    let serial = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();
    let opt = freemdu_home::new_optical_port(peripherals.UART1).unwrap();

    spawner.spawn(led_task(led)).unwrap();
    spawner.spawn(bridge_task(serial, opt)).unwrap();
}
