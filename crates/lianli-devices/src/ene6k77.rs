//! ENE 6K77 wired fan controller driver (SL/AL series).
//!
//! VID=0x0CF2, PID=0xA100-0xA106
//!
//! Protocol uses HID Feature Reports with Report ID 0xE0.
//! Each controller has 4 fan groups with independent PWM duty control.
//! RPM is read via feature report 0x50 sub-command 0x00.

use crate::traits::FanDevice;
use anyhow::{bail, Context, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

const REPORT_ID: u8 = 0xE0;
const CMD_DELAY: Duration = Duration::from_millis(20);

/// ENE 6K77 model variant, determined by PID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ene6k77Model {
    /// 0xA100 — SL Fan (4 groups, 4 fans max each)
    SlFan,
    /// 0xA101 — AL Fan (4 groups, dual-ring LEDs)
    AlFan,
    /// 0xA102 — SL Infinity (4 groups)
    SlInfinity,
    /// 0xA103 — SL V2 Fan (4 groups, 6 fans max each)
    SlV2Fan,
    /// 0xA104 — AL V2 Fan (4 groups, 6 fans max each)
    AlV2Fan,
    /// 0xA105 — SL V2A Fan
    SlV2aFan,
    /// 0xA106 — SL Redragon
    SlRedragon,
}

impl Ene6k77Model {
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            0xA100 => Some(Self::SlFan),
            0xA101 => Some(Self::AlFan),
            0xA102 => Some(Self::SlInfinity),
            0xA103 => Some(Self::SlV2Fan),
            0xA104 => Some(Self::AlV2Fan),
            0xA105 => Some(Self::SlV2aFan),
            0xA106 => Some(Self::SlRedragon),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::SlFan => "SL Fan",
            Self::AlFan => "AL Fan",
            Self::SlInfinity => "SL Infinity",
            Self::SlV2Fan => "SL V2 Fan",
            Self::AlV2Fan => "AL V2 Fan",
            Self::SlV2aFan => "SL V2A Fan",
            Self::SlRedragon => "SL Redragon",
        }
    }

    /// Whether this is a V2 model (supports 6 fans/group, 9-byte RPM response).
    pub fn is_v2(&self) -> bool {
        matches!(self, Self::SlV2Fan | Self::AlV2Fan | Self::SlV2aFan)
    }

    /// Whether this is an AL-style model (different set-quantity command format).
    pub fn is_al(&self) -> bool {
        matches!(self, Self::AlFan | Self::AlV2Fan)
    }

    /// Max fans per group.
    pub fn max_fans_per_group(&self) -> u8 {
        if self.is_v2() { 6 } else { 4 }
    }
}

/// Firmware version info read from the device.
#[derive(Debug, Clone)]
pub struct Ene6k77Firmware {
    pub customer_id: u8,
    pub project_id: u8,
    pub major_id: u8,
    pub minor_id: u8,
    pub fine_tune: u8,
}

impl std::fmt::Display for Ene6k77Firmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = if self.fine_tune < 8 {
            "1.0".to_string()
        } else {
            let v = ((self.fine_tune >> 4) * 10 + (self.fine_tune & 0x0F) + 2) as f32 / 10.0;
            format!("{v:.1}")
        };
        write!(
            f,
            "v{} (cust={:#04x} proj={:#04x} major={:#04x} minor={:#04x})",
            version, self.customer_id, self.project_id, self.major_id, self.minor_id
        )
    }
}

/// ENE 6K77 fan controller.
///
/// Wraps an opened HID device and provides fan speed control + RPM reading.
/// Does NOT touch RGB/LED effects — that's OpenRGB's domain.
pub struct Ene6k77Controller {
    device: Mutex<HidDevice>,
    model: Ene6k77Model,
    pid: u16,
    firmware: Option<Ene6k77Firmware>,
    /// Number of fans configured per group [group0, group1, group2, group3].
    fan_quantities: [u8; 4],
}

impl Ene6k77Controller {
    /// Open an ENE 6K77 controller by HID device handle and PID.
    pub fn new(device: HidDevice, pid: u16) -> Result<Self> {
        let model = Ene6k77Model::from_pid(pid)
            .ok_or_else(|| anyhow::anyhow!("Unknown ENE 6K77 PID: {pid:#06x}"))?;

        let mut ctrl = Self {
            device: Mutex::new(device),
            model,
            pid,
            firmware: None,
            fan_quantities: [0; 4],
        };

        ctrl.initialize()?;
        Ok(ctrl)
    }

    /// Initialize the controller: read firmware version.
    fn initialize(&mut self) -> Result<()> {
        info!(
            "Initializing ENE 6K77 {} (PID={:#06x})",
            self.model.name(),
            self.pid
        );

        match self.read_firmware() {
            Ok(fw) => {
                info!("  Firmware: {fw}");
                self.firmware = Some(fw);
            }
            Err(e) => {
                warn!("  Failed to read firmware: {e}");
            }
        }

        Ok(())
    }

    /// Read firmware version from the device.
    fn read_firmware(&self) -> Result<Ene6k77Firmware> {
        self.send_feature(&[REPORT_ID, 0x50, 0x01])?;
        let data = self.read_input(5)?;
        Ok(Ene6k77Firmware {
            customer_id: data[0],
            project_id: data[1],
            major_id: data[2],
            minor_id: data[3],
            fine_tune: data[4],
        })
    }

    /// Set fan quantity for a group.
    ///
    /// This tells the controller how many fans are connected to each group,
    /// which affects RPM reporting accuracy.
    pub fn set_fan_quantity(&mut self, group: u8, quantity: u8) -> Result<()> {
        if group >= 4 {
            bail!("Group index {group} out of range (0-3)");
        }
        let max = self.model.max_fans_per_group();
        let qty = quantity.min(max);

        let cmd = if self.model.is_v2() {
            if self.model.is_al() {
                // ALV2: [0xE0, 0x10, 0x60, groupIndex+1, quantity, 0x00]
                vec![REPORT_ID, 0x10, 0x60, group + 1, qty, 0x00]
            } else {
                // SLV2/SLV2A: [0xE0, 0x10, 0x60, (groupIndex << 4) | quantity]
                vec![REPORT_ID, 0x10, 0x60, (group << 4) | (qty & 0x0F)]
            }
        } else if self.model.is_al() {
            // AL: [0xE0, 0x10, 0x40, groupIndex+1, quantity, 0x00]
            vec![REPORT_ID, 0x10, 0x40, group + 1, qty, 0x00]
        } else {
            // SL/SL Infinity/Redragon: [0xE0, 0x10, 0x32, (groupIndex << 4) | quantity]
            vec![REPORT_ID, 0x10, 0x32, (group << 4) | (qty & 0x0F)]
        };

        self.send_feature(&cmd)?;
        self.fan_quantities[group as usize] = qty;
        debug!(
            "Set group {group} fan quantity to {qty} (model={})",
            self.model.name()
        );
        thread::sleep(CMD_DELAY);
        Ok(())
    }

    /// Read RPM values for all 4 groups.
    ///
    /// Returns [group0_rpm, group1_rpm, group2_rpm, group3_rpm].
    pub fn read_rpms(&self) -> Result<[u16; 4]> {
        self.send_feature(&[REPORT_ID, 0x50, 0x00])?;

        let mut rpms = [0u16; 4];

        if self.model.is_v2() {
            // V2 models return 9 bytes (1 padding + 4x2 RPM)
            let data = self.read_input(9)?;
            for i in 0..4 {
                let offset = 1 + i * 2;
                rpms[i] = u16::from_be_bytes([data[offset], data[offset + 1]]);
            }
        } else {
            // Standard models return 8 bytes (4x2 RPM)
            let data = self.read_input(8)?;
            for i in 0..4 {
                let offset = i * 2;
                rpms[i] = u16::from_be_bytes([data[offset], data[offset + 1]]);
            }
        }

        Ok(rpms)
    }

    /// Set fan speed (PWM duty) for a single group.
    ///
    /// `group`: 0-3
    /// `duty`: 0-255 (0% to 100%)
    pub fn set_group_speed(&self, group: u8, duty: u8) -> Result<()> {
        if group >= 4 {
            bail!("Group index {group} out of range (0-3)");
        }

        // [0xE0, 0x2G, 0x00, DUTY] where G = group index
        self.send_feature(&[REPORT_ID, 0x20 | group, 0x00, duty])?;
        debug!("Set group {group} speed to duty={duty} ({:.0}%)", duty as f32 / 2.55);
        thread::sleep(CMD_DELAY);
        Ok(())
    }

    /// Set fan speeds for all 4 groups at once.
    pub fn set_all_speeds(&self, duties: &[u8; 4]) -> Result<()> {
        for (group, &duty) in duties.iter().enumerate() {
            self.set_group_speed(group as u8, duty)?;
        }
        Ok(())
    }

    pub fn model(&self) -> Ene6k77Model {
        self.model
    }

    pub fn pid(&self) -> u16 {
        self.pid
    }

    pub fn firmware(&self) -> Option<&Ene6k77Firmware> {
        self.firmware.as_ref()
    }

    // -- Low-level HID helpers --

    fn send_feature(&self, data: &[u8]) -> Result<()> {
        let dev = self.device.lock();
        dev.send_feature_report(data)
            .context("ENE 6K77: send feature report")?;
        Ok(())
    }

    fn read_input(&self, expected_len: usize) -> Result<Vec<u8>> {
        let dev = self.device.lock();
        let mut buf = vec![0u8; expected_len + 1]; // +1 for report ID
        let n = dev
            .read_timeout(&mut buf, 100)
            .context("ENE 6K77: read input report")?;
        if n < expected_len {
            bail!(
                "ENE 6K77: expected {expected_len} bytes, got {n}"
            );
        }
        // Skip report ID byte if present
        if buf[0] == REPORT_ID && n > expected_len {
            Ok(buf[1..=expected_len].to_vec())
        } else {
            Ok(buf[..expected_len].to_vec())
        }
    }
}

impl FanDevice for Ene6k77Controller {
    fn set_fan_speed(&self, slot: u8, duty: u8) -> Result<()> {
        self.set_group_speed(slot, duty)
    }

    fn set_fan_speeds(&self, duties: &[u8]) -> Result<()> {
        for (i, &duty) in duties.iter().take(4).enumerate() {
            self.set_group_speed(i as u8, duty)?;
        }
        Ok(())
    }

    fn read_fan_rpm(&self) -> Result<Vec<u16>> {
        Ok(self.read_rpms()?.to_vec())
    }

    fn fan_slot_count(&self) -> u8 {
        4
    }
}
