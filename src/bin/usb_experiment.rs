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

use core::convert::TryInto;
use core::fmt::Write;
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

#[repr(C)]
#[repr(packed(1))]
#[allow(non_snake_case)]
struct DeviceDescriptor {
    bLength: u8,
    bDescriptorType: u8,
    bcdUsb: u16,
    bDeviceClass: u8,
    bDeviceSubClass: u8,
    bDeviceProtocol: u8,
    bMaxPacketSize0: u8,
    idVendor: u16,
    idProduct: u16,
    bcdDevice: u16,
    iManufacturer: u8,
    iProduct: u8,
    iSerialNumber: u8,
    bNumConfigurations: u8,
}

static DEVICE: DeviceDescriptor = DeviceDescriptor {
    bLength: core::mem::size_of::<DeviceDescriptor>() as u8,
    bDescriptorType: 1, // constant from spec
    bcdUsb: 0x0200,
    bDeviceClass: 0,
    bDeviceSubClass: 0,
    bDeviceProtocol: 0,
    bMaxPacketSize0: 64,
    idVendor: 0x1337,
    idProduct: 0x55aa,
    bcdDevice: 0x0001,
    iManufacturer: 0,
    iProduct: 0,
    iSerialNumber: 0,
    bNumConfigurations: 0,
};

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
type Uart = serial::Serial<
    UART0,
    PA1<AlternateFunction<AF1, PushPull>>,
    PA0<AlternateFunction<AF1, PushPull>>,
    (),
    (),
>;
static mut UART: *mut Uart = 0 as *mut _;
static mut RED_LED: *mut PF1<Output<PushPull>> = 0 as *mut _;
static mut GREEN_LED: *mut PF3<Output<PushPull>> = 0 as *mut _;
static mut TIMER: *mut tm4c123x::WTIMER0 = 0 as *mut _;

enum UsbSetupState {
    Normal,
    PendingSetAddress(u8),
}
static mut STATE: UsbSetupState = UsbSetupState::Normal;

#[interrupt]
unsafe fn USB0() {
    let usb0 = &*tm4c123x::USB0::ptr();
    let uart = &mut *UART;
    let timer = &mut *TIMER;

    let start = timer.tar.read().bits();

    let is = usb0.is.read().bits();
    let rxis = usb0.rxis.read().bits();
    let txis = usb0.txis.read().bits();
    writeln!(
        uart,
        "is: 0x{:02x}, rxis: 0x{:04x}, txis: 0x{:04x}",
        is, rxis, txis
    )
    .unwrap();
    writeln!(uart, "address is currently {}", (*usb0).faddr.read().bits()).unwrap();

    if txis & 0x1 != 0 {
        do_endpoint_0(usb0, uart);
    }

    if is & 0x40 != 0 {
        // reset
        (*usb0).faddr.write(|w| w.bits(0));
        STATE = UsbSetupState::Normal;
    }

    writeln!(uart).unwrap();

    match () {
        () if is & 0x4 != 0 => (&mut *RED_LED).set_high().unwrap(),
        () if txis & 0x1 != 0 => (&mut *RED_LED).set_low().unwrap(),
        _ => (&mut *GREEN_LED).set_high().unwrap(),
    }

    let end = timer.tar.read().bits();
    writeln!(
        uart,
        "It took {} clocks to run the interrupt\n\n",
        end - start
    )
    .unwrap();
}

#[allow(non_snake_case)]
unsafe fn do_endpoint_0(usb: &tm4c123x::usb0::RegisterBlock, uart: &mut Uart) {
    let csrl0 = usb.csrl0.read();
    writeln!(uart, "csrl0: 0x{:0x}", csrl0.bits()).unwrap();

    if let UsbSetupState::PendingSetAddress(addr) = STATE {
        if !csrl0.dataend().bit() {
            // Now we know that hardware has completed the Status Stage of the Set Address command,
            // because we had set this bit when we put PendingAddress into STATE.  The hardware
            // dutifully sends us an interrupt with ALL of the status bits cleared when this
            // happens.
            writeln!(uart, "setting address to {}", addr).unwrap();
            usb.faddr.write(|w| w.faddr().bits(addr as u8));
            STATE = UsbSetupState::Normal;
        }
    }

    if csrl0.rxrdy().bit() {
        writeln!(uart, "I got a packet!").unwrap();

        let mut packet_buffer = [0u8; 64];
        let count = usb.count0.read().count().bits() as usize;

        for i in 0..count {
            let addr = &usb.fifo0 as *const _ as *const u8;
            let byte = core::ptr::read_volatile(addr);
            packet_buffer[i] = byte;
        }

        let packet = &packet_buffer[0..count];

        writeln!(uart, "{:02x?}", packet).unwrap();

        if packet.len() != 8 {
            return; // TODO: stall?
        }

        let bmRequestType = packet[0];
        let bRequest = packet[1];
        let wValue = u16::from_le_bytes(packet[2..4].try_into().unwrap());
        let wIndex = u16::from_le_bytes(packet[4..6].try_into().unwrap());
        let wLength = u16::from_le_bytes(packet[6..8].try_into().unwrap());

        match (bmRequestType, bRequest, wValue, wIndex, wLength) {
            (0x80, 6, descriptor, 0, length) => {
                usb.csrl0.modify(|_r, w| w.rxrdyc().set_bit());
                let slice = get_descriptor(descriptor);
                if let Some(to_send) = slice {
                    let fifo = &usb.fifo0 as *const _ as *mut u8;
                    for i in &to_send[0..core::cmp::min(length as usize, to_send.len())] {
                        core::ptr::write_volatile(fifo, *i);
                    }
                    usb.csrl0.modify(|_r, w| {
                        w.dataend().set_bit();
                        w.txrdy().set_bit()
                    });
                } else {
                    // we don't have this descriptor, so fail the request
                    writeln!(uart, "no descriptor for type {:x?}", descriptor).unwrap();
                    usb.csrl0.modify(|_r, w| w.stall().set_bit());
                }
            }
            (0x0, 5, addr, 0, 0) => {
                // I think setting DATAEND is going to make the hardware send a zero-byte DATA1
                // response to the status stage IN on our behalf, which will conclude the
                // transaction.

                usb.csrl0.modify(|_r, w| {
                    w.rxrdyc().set_bit();
                    w.dataend().set_bit()
                });
                STATE = UsbSetupState::PendingSetAddress(addr as u8);
            }
            x => {
                writeln!(uart, "Unknown request: {:x?}", x).unwrap();
            }
        }
    }

    if csrl0.stalled().bit() {
        // "Software must clear this bit", I suppose we get interrupted with this set when the
        // hardware finally does respond to our STALL request.
        usb.csrl0.modify(|_r, w| w.stalled().clear_bit());
    }
}

unsafe fn get_descriptor(id: u16) -> Option<&'static [u8]> {
    match id {
        // device descriptor
        0x0100 => Some(make_slice_of(&DEVICE)),
        _ => None,
    }
}

unsafe fn make_slice_of<'a, T>(object: &'a T) -> &'a [u8] {
    core::slice::from_raw_parts(object as *const _ as *const u8, core::mem::size_of::<T>())
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
        UART = &mut uart;
        GREEN_LED = &mut board.led_green;
        RED_LED = &mut board.led_red;
    }

    stellaris_launchpad::cpu::sysctl::control_power(
        &board.power_control,
        stellaris_launchpad::cpu::sysctl::Domain::WideTimer0,
        stellaris_launchpad::cpu::sysctl::RunMode::Run,
        stellaris_launchpad::cpu::sysctl::PowerState::On,
    );
    board.WTIMER0.cfg.write(|w| unsafe { w.cfg().bits(4) });
    board.WTIMER0.tamr.write(|w| {
        w.tacdir().set_bit();
        unsafe { w.tamr().bits(2) }
    });
    board.WTIMER0.ctl.write(|w| w.taen().set_bit());
    unsafe {
        TIMER = &mut board.WTIMER0;
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
            cortex_m::asm::wfi();
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
