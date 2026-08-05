#![no_std]
#![no_main]

extern crate alloc;

use esp_backtrace as _;
use esp_hal::{
    gpio::{Level, Output, OutputConfig},
    timer::timg::TimerGroup,
};
use freemdu_home;

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

    // UART1 inicializálása.
    // Ez konfigurálja a GPIO20-at UART RX-nek.
    let _port = freemdu_home::new_optical_port(peripherals.UART1).unwrap();

    // GPIO10 kimenet
    let mut gpio10 = Output::new(
        peripherals.GPIO10,
        Level::Low,
        OutputConfig::default(),
    );

    // GPIO regiszter blokk
    let gpio = esp_hal::peripherals::GPIO::regs();

    loop {
        // GPIO20 állapotának kiolvasása
        let level = (gpio.in_().read().bits() & (1 << 20)) != 0;

        if level {
            gpio10.set_high();
        } else {
            gpio10.set_low();
        }
    }
}
