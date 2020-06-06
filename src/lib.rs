#![no_std]

use core::num::NonZeroU16;
use stellaris_launchpad::cpu::gpio::gpiod::{GpioControl, PD4, PD5};
use usb_device::bus::PollResult;
use usb_device::endpoint::EndpointAddress;
use usb_device::Result;

pub struct USB {
    device: tm4c123x::USB0,
    // the device has 7 RX and 7 TX endpoints, each numbered 1-7.  The corresponding (endpoint-1)th
    // index in this array will become Some when it is allocated.
    max_packet_size_out: [Option<NonZeroU16>; 7],
    max_packet_size_in: [Option<NonZeroU16>; 7],
}

unsafe impl Sync for USB {}

impl usb_device::bus::UsbBus for USB {
    fn alloc_ep(
        &mut self,
        ep_dir: usb_device::UsbDirection,
        ep_addr: Option<EndpointAddress>,
        _ep_type: usb_device::endpoint::EndpointType,
        max_packet_size: u16,
        _interval: u8,
    ) -> usb_device::Result<EndpointAddress> {
        let endpoints = match ep_dir {
            usb_device::UsbDirection::In => &mut self.max_packet_size_in,
            usb_device::UsbDirection::Out => &mut self.max_packet_size_out,
        };
        let chosen_endpoint = match ep_addr {
            // if a particular endpoint number was requested AND it is currently available
            Some(requested)
                if requested.index() > 0 && endpoints[requested.index() - 1].is_none() =>
            {
                requested.index() - 1
            }
            // otherwise, look for a None anywhere in the array and use its index.
            _ => match endpoints.iter().enumerate().find(|&(_i, v)| v.is_none()) {
                Some((i, _)) => i,
                _ => return Err(usb_device::UsbError::EndpointOverflow),
            },
        };
        endpoints[chosen_endpoint] =
            Some(unsafe { NonZeroU16::new_unchecked(core::cmp::max(1, max_packet_size)) });

        Ok(EndpointAddress::from_parts(chosen_endpoint + 1, ep_dir))
    }

    fn enable(&mut self) {
        self.device.power.modify(|_r, w| w.softconn().set_bit());
    }

    fn reset(&self) {
        let mut fifo_address = 64; // after the endpoint 0 fifo
        for (i, m) in self.max_packet_size_out.iter().enumerate() {
            let m = match m {
                None => continue,
                m => m,
            };
            let m = m.unwrap();
            let (size_setting, fifo_size) = size_setting_from_requested_size(m);
            self.device
                .epidx
                .write(|w| unsafe { w.epidx().bits(i as u8 + 1) });
            self.device.rxfifosz.write(|w| {
                w.dpb().clear_bit();
                unsafe { w.size().bits(size_setting) }
            });
            self.device
                .rxfifoadd
                .write(|w| unsafe { w.addr().bits(fifo_address / 8) });
            unsafe {
                match i {
                    0 => self.device.rxmaxp1.write(|w| w.maxload().bits(m.get())),
                    1 => self.device.rxmaxp2.write(|w| w.maxload().bits(m.get())),
                    2 => self.device.rxmaxp3.write(|w| w.maxload().bits(m.get())),
                    3 => self.device.rxmaxp4.write(|w| w.maxload().bits(m.get())),
                    4 => self.device.rxmaxp5.write(|w| w.maxload().bits(m.get())),
                    5 => self.device.rxmaxp6.write(|w| w.maxload().bits(m.get())),
                    6 => self.device.rxmaxp7.write(|w| w.maxload().bits(m.get())),
                    _ => panic!("the endpoint array only has 7 elements"),
                }
            }
            fifo_address += fifo_size;
        }
        for (i, m) in self.max_packet_size_in.iter().enumerate() {
            let m = match m {
                None => continue,
                m => m,
            };
            let m = m.unwrap();
            let (size_setting, fifo_size) = size_setting_from_requested_size(m);
            self.device
                .epidx
                .write(|w| unsafe { w.epidx().bits(i as u8 + 1) });
            self.device.txfifosz.write(|w| {
                w.dpb().clear_bit();
                unsafe { w.size().bits(size_setting) }
            });
            self.device
                .txfifoadd
                .write(|w| unsafe { w.addr().bits(fifo_address / 8) });
            unsafe {
                match i {
                    0 => self.device.txmaxp1.write(|w| w.maxload().bits(m.get())),
                    1 => self.device.txmaxp2.write(|w| w.maxload().bits(m.get())),
                    2 => self.device.txmaxp3.write(|w| w.maxload().bits(m.get())),
                    3 => self.device.txmaxp4.write(|w| w.maxload().bits(m.get())),
                    4 => self.device.txmaxp5.write(|w| w.maxload().bits(m.get())),
                    5 => self.device.txmaxp6.write(|w| w.maxload().bits(m.get())),
                    6 => self.device.txmaxp7.write(|w| w.maxload().bits(m.get())),
                    _ => panic!("the endpoint array only has 7 elements"),
                }
            }
            fifo_address += fifo_size;
        }
    }

    fn set_device_address(&self, addr: u8) {
        unimplemented!()
    }

    fn write(&self, ep: EndpointAddress, buf: &[u8]) -> Result<usize> {
        unimplemented!()
    }

    fn read(&self, ep: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
    }

    fn set_stalled(&self, ep: EndpointAddress, stalled: bool) {
        unimplemented!()
    }

    fn is_stalled(&self, ep: EndpointAddress) -> bool {
        unimplemented!()
    }

    fn suspend(&self) {
        unimplemented!()
    }

    fn resume(&self) {
        unimplemented!()
    }

    fn poll(&self) -> PollResult {
        let is = self.device.is.read();
        if is.reset().bit() {
            return PollResult::Reset;
        }
        if is.suspend().bit() {
            return PollResult::Suspend;
        }
        if is.resume().bit() {
            return PollResult::Resume;
        }
        PollResult::None
    }
}

impl USB {
    pub fn new<ModeM, ModeP>(
        usb0: tm4c123x::USB0,
        dminus: PD4<ModeM>,
        dplus: PD5<ModeP>,
        gpio_control: &mut GpioControl,
        power_control: &stellaris_launchpad::cpu::sysctl::PowerControl,
    ) -> usb_device::bus::UsbBusAllocator<USB> {
        use stellaris_launchpad::cpu::sysctl::{control_power, reset, Domain, PowerState, RunMode};
        control_power(power_control, Domain::Usb, RunMode::Run, PowerState::On);
        reset(power_control, Domain::Usb);

        unsafe {
            // since I hold a reference to PowerControl, this should not clobber anything
            let sysctl = &*tm4c123x::SYSCTL::ptr();
            sysctl.rcc2.modify(|_r, w| w.usbpwrdn().clear_bit());

            // since I hold a unique reference to gpiod::GpioControl, this should also not stomp on anything
            let portd = &*tm4c123x::GPIO_PORTD::ptr();
            portd.amsel.modify(|r, w| {
                w.bits(
                    r.bits() | 0x30, /* bits 4 and 5 correspond to pin D4 and D5 */
                )
            });
        }

        let this = USB {
            device: usb0,
            max_packet_size_out: [None; 7],
            max_packet_size_in: [None; 7],
        };
        usb_device::bus::UsbBusAllocator::new(this)
    }
}

fn size_setting_from_requested_size(requested: NonZeroU16) -> (u8, u16) {
    let requested = requested.get();
    if requested <= 8 {
        return (0x0, 8);
    }
    if requested <= 16 {
        return (0x1, 16);
    }
    if requested <= 32 {
        return (0x2, 32);
    }
    if requested <= 64 {
        return (0x3, 64);
    }
    if requested <= 128 {
        return (0x4, 128);
    }
    if requested <= 256 {
        return (0x5, 256);
    }
    if requested <= 512 {
        return (0x6, 512);
    }
    if requested <= 1024 {
        return (0x7, 1024);
    }
    if requested <= 2048 {
        return (0x8, 2048);
    }

    panic!("the USB peripheral does not support packet sizes larger than 2048");
}
