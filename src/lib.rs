#![no_std]

use core::num::NonZeroU16;
use stellaris_launchpad::cpu::gpio::gpiod::{GpioControl, PD4, PD5};
use usb_device::bus::PollResult;
use usb_device::endpoint::EndpointAddress;
use usb_device::{Result, UsbDirection, UsbError};

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
        self.device.faddr.write(|w| unsafe { w.bits(addr) });
    }

    fn write(&self, ep: EndpointAddress, buf: &[u8]) -> Result<usize> {
        if ep.direction() != UsbDirection::In {
            return Err(UsbError::InvalidEndpoint);
        }
        if ep.index() != 0 && self.max_packet_size_in[ep.index() as usize - 1].is_none() {
            // was not previously allocated
            return Err(UsbError::InvalidEndpoint);
        }
        let (fifo_p, already_queued, maxp) = match ep.index() {
            0 => (
                &self.device.fifo0 as *const _ as *mut u8,
                self.device.csrl0.read().txrdy().bit(),
                64,
            ),
            1 => (
                &self.device.fifo1 as *const _ as *mut u8,
                self.device.txcsrl1.read().txrdy().bit(),
                self.device.txmaxp1.read().bits(),
            ),
            2 => (
                &self.device.fifo2 as *const _ as *mut u8,
                self.device.txcsrl2.read().txrdy().bit(),
                self.device.txmaxp2.read().bits(),
            ),
            3 => (
                &self.device.fifo3 as *const _ as *mut u8,
                self.device.txcsrl3.read().txrdy().bit(),
                self.device.txmaxp3.read().bits(),
            ),
            4 => (
                &self.device.fifo4 as *const _ as *mut u8,
                self.device.txcsrl4.read().txrdy().bit(),
                self.device.txmaxp4.read().bits(),
            ),
            5 => (
                &self.device.fifo5 as *const _ as *mut u8,
                self.device.txcsrl5.read().txrdy().bit(),
                self.device.txmaxp5.read().bits(),
            ),
            6 => (
                &self.device.fifo6 as *const _ as *mut u8,
                self.device.txcsrl6.read().txrdy().bit(),
                self.device.txmaxp6.read().bits(),
            ),
            7 => (
                &self.device.fifo7 as *const _ as *mut u8,
                self.device.txcsrl7.read().txrdy().bit(),
                self.device.txmaxp7.read().bits(),
            ),
            _ => panic!("the device only has 7 IN endpoints"),
        };
        if buf.len() > maxp as usize {
            return Err(UsbError::BufferOverflow);
        }
        if already_queued {
            return Err(UsbError::WouldBlock);
        }

        for c in buf {
            unsafe {
                core::ptr::write_volatile(fifo_p, *c);
            }
        }

        match ep.index() {
            0 => self.device.csrl0.modify(|_r, w| w.txrdy().set_bit()),
            1 => self.device.txcsrl1.modify(|_r, w| w.txrdy().set_bit()),
            2 => self.device.txcsrl2.modify(|_r, w| w.txrdy().set_bit()),
            3 => self.device.txcsrl3.modify(|_r, w| w.txrdy().set_bit()),
            4 => self.device.txcsrl4.modify(|_r, w| w.txrdy().set_bit()),
            5 => self.device.txcsrl5.modify(|_r, w| w.txrdy().set_bit()),
            6 => self.device.txcsrl6.modify(|_r, w| w.txrdy().set_bit()),
            7 => self.device.txcsrl7.modify(|_r, w| w.txrdy().set_bit()),
            _ => panic!("we would've already panicked"),
        };
        Ok(buf.len())
    }

    fn read(&self, ep: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        if ep.direction() != UsbDirection::Out {
            return Err(UsbError::InvalidEndpoint);
        }
        if ep.index() != 0 && self.max_packet_size_out[ep.index() as usize - 1].is_none() {
            // was not previously allocated
            return Err(UsbError::InvalidEndpoint);
        }
        let (fifo_p, available, fifo_bytes) = match ep.index() {
            0 => (
                &self.device.fifo0 as *const _ as *mut u8,
                self.device.csrl0.read().rxrdy().bit(),
                self.device.count0.read().bits() as u16,
            ),
            1 => (
                &self.device.fifo1 as *const _ as *mut u8,
                self.device.rxcsrl1.read().rxrdy().bit(),
                self.device.rxcount1.read().bits(),
            ),
            2 => (
                &self.device.fifo2 as *const _ as *mut u8,
                self.device.rxcsrl2.read().rxrdy().bit(),
                self.device.rxcount1.read().bits(),
            ),
            3 => (
                &self.device.fifo3 as *const _ as *mut u8,
                self.device.rxcsrl3.read().rxrdy().bit(),
                self.device.rxcount3.read().bits(),
            ),
            4 => (
                &self.device.fifo4 as *const _ as *mut u8,
                self.device.rxcsrl4.read().rxrdy().bit(),
                self.device.rxcount4.read().bits(),
            ),
            5 => (
                &self.device.fifo5 as *const _ as *mut u8,
                self.device.rxcsrl5.read().rxrdy().bit(),
                self.device.rxcount5.read().bits(),
            ),
            6 => (
                &self.device.fifo6 as *const _ as *mut u8,
                self.device.rxcsrl6.read().rxrdy().bit(),
                self.device.rxcount6.read().bits(),
            ),
            7 => (
                &self.device.fifo7 as *const _ as *mut u8,
                self.device.rxcsrl7.read().rxrdy().bit(),
                self.device.rxcount7.read().bits(),
            ),
            _ => panic!("the device only has 7 IN endpoints"),
        };
        if buf.len() < fifo_bytes as usize {
            return Err(UsbError::BufferOverflow);
        }
        if !available {
            return Err(UsbError::WouldBlock);
        }

        for i in 0..fifo_bytes {
            unsafe {
                buf[i as usize] = core::ptr::read_volatile(fifo_p);
            }
        }

        match ep.index() {
            0 => self.device.csrl0.modify(|_r, w| w.rxrdyc().set_bit()),
            1 => self.device.rxcsrl1.modify(|_r, w| w.rxrdy().clear_bit()),
            2 => self.device.rxcsrl2.modify(|_r, w| w.rxrdy().clear_bit()),
            3 => self.device.rxcsrl3.modify(|_r, w| w.rxrdy().clear_bit()),
            4 => self.device.rxcsrl4.modify(|_r, w| w.rxrdy().clear_bit()),
            5 => self.device.rxcsrl5.modify(|_r, w| w.rxrdy().clear_bit()),
            6 => self.device.rxcsrl6.modify(|_r, w| w.rxrdy().clear_bit()),
            7 => self.device.rxcsrl7.modify(|_r, w| w.rxrdy().clear_bit()),
            _ => panic!("we would've already panicked"),
        };
        Ok(buf.len())
    }

    fn set_stalled(&self, ep: EndpointAddress, stalled: bool) {
        match (ep.direction(), ep.index()) {
            (_, 0) => self.device.csrl0.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 1) => self.device.txcsrl1.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 2) => self.device.txcsrl2.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 3) => self.device.txcsrl3.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 4) => self.device.txcsrl4.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 5) => self.device.txcsrl5.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 6) => self.device.txcsrl6.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::In, 7) => self.device.txcsrl7.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 1) => self.device.rxcsrl1.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 2) => self.device.rxcsrl2.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 3) => self.device.rxcsrl3.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 4) => self.device.rxcsrl4.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 5) => self.device.rxcsrl5.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 6) => self.device.rxcsrl6.modify(|_r, w| w.stall().bit(stalled)),
            (UsbDirection::Out, 7) => self.device.rxcsrl7.modify(|_r, w| w.stall().bit(stalled)),
            (_, _) => panic!("set_stalled: invalid endpoint for hardware: {:?}", ep),
        }
    }

    fn is_stalled(&self, ep: EndpointAddress) -> bool {
        match (ep.direction(), ep.index()) {
            (_, 0) => self.device.csrl0.read().stall().bit(),
            (UsbDirection::In, 1) => self.device.txcsrl1.read().stall().bit(),
            (UsbDirection::In, 2) => self.device.txcsrl2.read().stall().bit(),
            (UsbDirection::In, 3) => self.device.txcsrl3.read().stall().bit(),
            (UsbDirection::In, 4) => self.device.txcsrl4.read().stall().bit(),
            (UsbDirection::In, 5) => self.device.txcsrl5.read().stall().bit(),
            (UsbDirection::In, 6) => self.device.txcsrl6.read().stall().bit(),
            (UsbDirection::In, 7) => self.device.txcsrl7.read().stall().bit(),
            (UsbDirection::Out, 1) => self.device.rxcsrl1.read().stall().bit(),
            (UsbDirection::Out, 2) => self.device.rxcsrl2.read().stall().bit(),
            (UsbDirection::Out, 3) => self.device.rxcsrl3.read().stall().bit(),
            (UsbDirection::Out, 4) => self.device.rxcsrl4.read().stall().bit(),
            (UsbDirection::Out, 5) => self.device.rxcsrl5.read().stall().bit(),
            (UsbDirection::Out, 6) => self.device.rxcsrl6.read().stall().bit(),
            (UsbDirection::Out, 7) => self.device.rxcsrl7.read().stall().bit(),
            (_, _) => panic!("is_stalled: invalid endpoint for hardware: {:?}", ep),
        }
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
