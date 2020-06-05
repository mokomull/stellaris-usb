#![no_std]

use core::num::NonZeroU16;
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
        unimplemented!()
    }

    fn reset(&self) {
        unimplemented!()
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
        unimplemented!()
    }
}

impl USB {
    pub fn new(usb0: tm4c123x::USB0) -> usb_device::bus::UsbBusAllocator<USB> {
        let this = USB {
            device: usb0,
            max_packet_size_out: [None; 7],
            max_packet_size_in: [None; 7],
        };
        usb_device::bus::UsbBusAllocator::new(this)
    }
}