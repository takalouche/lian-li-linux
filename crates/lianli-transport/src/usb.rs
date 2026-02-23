use crate::error::TransportError;
use rusb::{Device, DeviceHandle, GlobalContext};
use std::time::Duration;
use tracing::{debug, info, warn};

pub const EP_OUT: u8 = 0x01;
pub const EP_IN: u8 = 0x81;
pub const USB_TIMEOUT: Duration = Duration::from_millis(5_000);
pub const LCD_WRITE_TIMEOUT: Duration = Duration::from_millis(10_000);

/// Low-level USB bulk transport wrapping a `rusb` device handle.
pub struct UsbTransport {
    handle: DeviceHandle<GlobalContext>,
    ep_out: u8,
    ep_in: u8,
}

impl UsbTransport {
    pub fn open(vid: u16, pid: u16) -> Result<Self, TransportError> {
        let handle = rusb::open_device_with_vid_pid(vid, pid)
            .ok_or(TransportError::DeviceNotFound { vid, pid })?;
        Ok(Self {
            handle,
            ep_out: EP_OUT,
            ep_in: EP_IN,
        })
    }

    pub fn open_device(device: Device<GlobalContext>) -> Result<Self, TransportError> {
        let handle = device.open()?;
        Ok(Self {
            handle,
            ep_out: EP_OUT,
            ep_in: EP_IN,
        })
    }

    pub fn detach_and_configure(&mut self, name: &str) -> Result<(), TransportError> {
        match self.handle.kernel_driver_active(0) {
            Ok(true) => {
                self.handle.detach_kernel_driver(0)?;
                debug!("Detached kernel driver from {name}");
            }
            Ok(false) => {}
            Err(rusb::Error::NotSupported) => {}
            Err(e) => return Err(e.into()),
        }

        match self.handle.set_active_configuration(1) {
            Ok(()) | Err(rusb::Error::Busy) | Err(rusb::Error::NotFound) => {}
            Err(rusb::Error::Io) => {
                warn!("{name} configuration I/O error, attempting USB reset");
                self.handle.reset()?;
                info!("{name} USB reset successful, retrying");
                std::thread::sleep(Duration::from_millis(500));
                match self.handle.set_active_configuration(1) {
                    Ok(()) | Err(rusb::Error::Busy) | Err(rusb::Error::NotFound) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }

        match self.handle.claim_interface(0) {
            Ok(()) => {
                let _ = self.handle.set_alternate_setting(0, 0);
            }
            Err(rusb::Error::Busy) => {
                warn!("{name} interface busy, attempting USB reset");
                self.handle.reset()?;
                info!("{name} USB reset successful");
                std::thread::sleep(Duration::from_millis(500));
                self.handle.claim_interface(0)?;
                let _ = self.handle.set_alternate_setting(0, 0);
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    pub fn write_bulk(&self, data: &[u8], timeout: Duration) -> Result<usize, TransportError> {
        Ok(self.handle.write_bulk(self.ep_out, data, timeout)?)
    }

    pub fn read_bulk(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, TransportError> {
        Ok(self.handle.read_bulk(self.ep_in, buf, timeout)?)
    }

    pub fn release(&self) {
        let _ = self.handle.release_interface(0);
    }

    pub fn reset(&self) -> Result<(), TransportError> {
        Ok(self.handle.reset()?)
    }

    pub fn inner(&self) -> &DeviceHandle<GlobalContext> {
        &self.handle
    }

    pub fn read_serial(&self, device: &Device<GlobalContext>) -> Option<String> {
        let desc = device.device_descriptor().ok()?;
        self.handle.read_serial_number_string_ascii(&desc).ok()
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(0);
    }
}

/// Find all USB devices matching a VID/PID, sorted by bus/address.
pub fn find_usb_devices(vid: u16, pid: u16) -> Result<Vec<Device<GlobalContext>>, TransportError> {
    let devices = rusb::devices()?;
    let mut list = Vec::new();
    for device in devices.iter() {
        if let Ok(desc) = device.device_descriptor() {
            if desc.vendor_id() == vid && desc.product_id() == pid {
                list.push(device);
            }
        }
    }
    list.sort_by_key(|dev| (dev.bus_number(), dev.address()));
    Ok(list)
}
