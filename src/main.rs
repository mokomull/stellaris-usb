#![no_std]
#![no_main]

use core::convert::TryInto;
use core::fmt::Write;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::digital::v2::OutputPin;
use stellaris_launchpad::cpu::gpio::{
    gpioa::{PA0, PA1},
    gpiof::{PF1, PF3},
    AlternateFunction, GpioExt, Output, PushPull, AF1,
};
use stellaris_launchpad::cpu::serial;
use stellaris_launchpad::cpu::time::Bps;
use tm4c123x::interrupt;
use tm4c123x::UART0;

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

    let mut delay = stellaris_launchpad::cpu::delay::Delay::new(
        board.core_peripherals.SYST,
        stellaris_launchpad::board::clocks(),
    );

    loop {
        delay.delay_ms(500u32);
        board.led_green.set_high().unwrap();
        delay.delay_ms(500u32);
        board.led_green.set_low().unwrap();
    }
}
