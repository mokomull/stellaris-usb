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

use core::fmt::Write;
use embedded_hal::digital::v2::OutputPin;
use stellaris_launchpad::cpu::time::Bps;
use stellaris_launchpad::cpu::serial;
use stellaris_launchpad::cpu::gpio::{GpioExt, Output, {gpiof::PF3}, PushPull};
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
static mut GREEN_LED: *mut PF3<Output<PushPull>> = 0 as *mut _;

#[interrupt]
unsafe fn USB0() {
    (&mut *GREEN_LED).set_high().unwrap();
}

#[no_mangle]
pub fn stellaris_main(mut board: stellaris_launchpad::board::Board) {
    let mut pins_a = board.GPIO_PORTA.split(&board.power_control);
    let mut uart = serial::Serial::uart0(
        board.UART0,
        pins_a.pa1.into_af_push_pull(&mut pins_a.control),
        pins_a.pa0.into_af_push_pull(&mut pins_a.control),
        (),
        (),
        Bps(115200),
        serial::NewlineMode::SwapLFtoCRLF,
        stellaris_launchpad::board::clocks(),
        &board.power_control,
    );

    unsafe {
        GREEN_LED = &mut board.led_green;
    }

    let sysctl = tm4c123x::SYSCTL::ptr();
    let gpiod = tm4c123x::GPIO_PORTD::ptr();
    let usb0 = tm4c123x::USB0::ptr();
    let nvic = tm4c123x::NVIC::ptr();
    unsafe {
        (*sysctl).rcgcusb.modify(|_r, w| w.r0().set_bit());
        (*sysctl).rcgcgpio.modify(|_r, w| w.r3().set_bit());
        (*sysctl).rcc2.modify(|_r, w| w.usbpwrdn().clear_bit());
        cortex_m::asm::delay(3); // let the clocks warm up

        // these bits are grey in the manual but I had a hunch I still needed to set them to make
        // the "analog" USB function work
        (*gpiod).amsel.modify(|r, w| w.bits(r.bits() | 0x30));
        (*usb0).power.modify(|_r, w| w.softconn().set_bit());


        (*nvic).iser[1].modify(|d| d | (1 << (44 - 32)));

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
