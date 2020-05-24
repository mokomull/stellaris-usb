//! A blinky-LED example application
//! This example uses launchpad-rs.

#![no_std]
#![no_main]
#![crate_type = "staticlib"]

// ****************************************************************************
//
// Imports
//
// ****************************************************************************

extern crate embedded_hal;
extern crate stellaris_launchpad;

use core::fmt::Write;
use stellaris_launchpad::cpu::{gpio, systick, timer, uart};
use embedded_hal::serial::Read as ReadHal;

// ****************************************************************************
//
// Public Types
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Private Types
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Public Data
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Public Functions
//
// ****************************************************************************

#[no_mangle]
pub extern "C" fn stellaris_main() {
    let mut uart = uart::Uart::new(uart::UartId::Uart0, 115200, uart::NewlineMode::SwapLFtoCRLF);
    let mut loops = 0;
    let mut ticks_last = systick::SYSTICK_MAX;
    let mut t = timer::Timer::new(timer::TimerId::Timer1A);
    t.enable_pwm(4096);

    let sysctl = tm4c123x::SYSCTL::ptr();
    let gpiod = tm4c123x::GPIO_PORTD::ptr();
    let usb0 = tm4c123x::USB0::ptr();
    unsafe {
        (*sysctl).rcgcusb.modify(|_r, w| w.r0().set_bit());
        (*sysctl).rcc2.modify(|_r, w| w.usbpwrdn().clear_bit());
        (*usb0).power.modify(|_r, w| w.softconn().set_bit());
        (*sysctl).rcgcgpio.modify(|_r, w| w.r3().set_bit());
        let gpiod_den = (*gpiod).den.read().bits();
        writeln!(uart, "got some bits").unwrap();
        writeln!(uart, "gpioden is 0x{:08x}", gpiod_den).unwrap();

        // *(0x4005_041c)

        loop{}
    }

    gpio::PinPort::PortF(gpio::Pin::Pin2).set_direction(gpio::PinMode::Peripheral);
    gpio::PinPort::PortF(gpio::Pin::Pin2).enable_ccp();
    let levels = [1u32, 256, 512, 1024, 2048, 4096];
    uart.write_all("Welcome to Launchpad Blink\n");
    loop {
        for level in levels.iter() {
            t.set_pwm(*level);
            let delta = systick::get_since(ticks_last);
            ticks_last = systick::get_ticks();
            writeln!(
                uart,
                "Hello, world! Loops = {}, elapsed = {}, run_time = {}, level = {}",
                loops,
                systick::ticks_to_usecs(delta),
                systick::run_time_us() as u32,
                level
            ).unwrap();
            while let Ok(ch) = uart.read() {
                writeln!(uart, "byte read {}", ch).unwrap();
            }
            loops = loops + 1;
            stellaris_launchpad::delay(250);
        }
    }
}

// ****************************************************************************
//
// Private Functions
//
// ****************************************************************************

// None

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
