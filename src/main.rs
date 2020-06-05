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
use usb_device::prelude::*;

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

    let bus = stellaris_usb::USB::new(board.USB0);
    let mut test_class = usb_device::test_class::TestClass::new(&bus);
    let mut usb_dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1337, 0xfeed))
        .product("stellaris-usb testing")
        .build();

    let mut counter = 0;
    loop {
        delay.delay_ms(1u32);
        usb_dev.poll(&mut [&mut test_class]);
        if counter == 0 {
            board.led_green.set_high().unwrap();
        } else if counter == 500 {
            board.led_green.set_low().unwrap();
        }
        counter += 1;
        counter %= 1000;
    }
}
