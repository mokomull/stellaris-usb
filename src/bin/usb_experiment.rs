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
use stellaris_launchpad::cpu::uart;
use tm4c123x::interrupt;

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

#[interrupt]
unsafe fn USB0() {
    stellaris_launchpad::board::led_on(stellaris_launchpad::board::Led::Green);
}

#[no_mangle]
pub extern "C" fn stellaris_main() {
    let mut uart = uart::Uart::new(uart::UartId::Uart0, 115200, uart::NewlineMode::SwapLFtoCRLF);

    let sysctl = tm4c123x::SYSCTL::ptr();
    let gpiod = tm4c123x::GPIO_PORTD::ptr();
    let usb0 = tm4c123x::USB0::ptr();
    unsafe {
        (*sysctl).rcgcusb.modify(|_r, w| w.r0().set_bit());
        (*sysctl).rcgcgpio.modify(|_r, w| w.r3().set_bit());
        (*sysctl).rcc2.modify(|_r, w| w.usbpwrdn().clear_bit());
        cortex_m::asm::delay(3); // let the clocks warm up

        // these bits are grey in the manual but I had a hunch I still needed to set them to make
        // the "analog" USB function work
        (*gpiod).amsel.modify(|r, w| w.bits(r.bits() | 0x30));
        (*usb0).power.modify(|_r, w| w.softconn().set_bit());

        writeln!(uart, "I did the thing").unwrap();

        loop {
            while !(*usb0).csrl0.read().setend().bit() {}

            writeln!(uart, "I got a control packet!").unwrap();

            let count = (*usb0).count0.read().count().bits();
            writeln!(uart, "It is {} bytes", count).unwrap();

            for _ in 0..count {
                let addr = &(*usb0).fifo0 as *const _ as *const u8;
                let byte = core::ptr::read_volatile(addr);
                write!(uart, "{:02x} ", byte).unwrap();
            }
            writeln!(uart).unwrap();
            writeln!(uart, "done").unwrap();

            (*usb0).csrl0.modify(|_r, w| {
                w.setendc().set_bit();
                w.stall().set_bit()
            });
            (*usb0).is.read();
            (*usb0).ie.modify(|_r, w| w.reset().set_bit());
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
