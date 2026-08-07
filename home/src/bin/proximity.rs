#![no_std]
#![no_main]

extern crate alloc;

use esp_backtrace as _;
use esp_hal::{
    gpio::{AnyPin,Level, Output, Input, InputConfig, OutputConfig},
    timer::timg::TimerGroup,
};

const TESTED_RX_PIN: u8 = match core::primitive::u8::from_str_radix(env!("PIN_OPTICAL_RX"), 10) {
    Ok(pin) => pin,
    Err(_) => panic!("A PIN_OPTICAL_RX is not valid."),
};
const TESTED_TX_PIN: u8 = match core::primitive::u8::from_str_radix(env!("PIN_OPTICAL_TX"), 10) {
    Ok(pin) => pin,
    Err(_) => panic!("A PIN_OPTICAL_RX is not valid."),
};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: embassy_executor::Spawner) {
    esp_alloc::heap_allocator!(size: 32 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    // RTOS
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // GPIO kimenet
    let _gpioforout = Output::new(
        unsafe { AnyPin::steal(TESTED_TX_PIN) },
        Level::High,
        OutputConfig::default(),
    );

    let _gpioforinput = Input::new(
        unsafe { AnyPin::steal(TESTED_RX_PIN) },
        InputConfig::default()
    );

    let mut gpio10 = Output::new(
        peripherals.GPIO10,
        Level::Low,
        OutputConfig::default(),
    );

    let gpios = esp_hal::peripherals::GPIO::regs();

    loop {

        let level = (gpios.in_().read().bits() & (1 << TESTED_RX_PIN)) != 0;

        if level {
            gpio10.set_high();
        } else {
            gpio10.set_low();
        }
    }
}
