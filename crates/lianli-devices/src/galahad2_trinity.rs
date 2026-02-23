//! Galahad II Trinity AIO driver (pump + fan control, NO LCD).
//!
//! VID=0x0416, PID=0x7371 (Performance) / 0x7373 (Regular)
//!
//! Protocol uses 64-byte HID output reports (Report ID 0x01) with 6-byte header.
//! Pump and fan PWM use 0-100% scale (not 0-255).
//! RPM is read via the handshake command (0x81).
//! No coolant temperature sensor — CPU temp must come from the system.

use crate::traits::{AioDevice, FanDevice};
use anyhow::{bail, Context, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use tracing::{debug, info, warn};

const REPORT_ID: u8 = 0x01;
const PACKET_SIZE: usize = 64;
const HEADER_LEN: usize = 6;
const READ_TIMEOUT_MS: i32 = 200;

// A-Commands
const CMD_HANDSHAKE: u8 = 0x81;
const CMD_GET_FIRMWARE: u8 = 0x86;
const CMD_SET_PUMP_PWM: u8 = 0x8A;
const CMD_SET_FAN_PWM: u8 = 0x8B;

/// Galahad2 Trinity model variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Galahad2TrinityModel {
    /// PID 0x7371 — Performance (pump RPM 2200-4200)
    Performance,
    /// PID 0x7373 — Regular (pump RPM 2200-3200)
    Regular,
}

impl Galahad2TrinityModel {
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            0x7371 => Some(Self::Performance),
            0x7373 => Some(Self::Regular),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Performance => "Galahad II Trinity Performance",
            Self::Regular => "Galahad II Trinity",
        }
    }
}

/// Handshake response data.
#[derive(Debug, Clone)]
pub struct Galahad2Handshake {
    pub fan_rpm: u16,
    pub pump_rpm: u16,
}

/// Galahad II Trinity AIO controller.
///
/// Provides pump + fan speed control. Does NOT have LCD or coolant temp sensor.
/// Does NOT touch RGB/LED effects — that's OpenRGB's domain.
pub struct Galahad2TrinityController {
    device: Mutex<HidDevice>,
    model: Galahad2TrinityModel,
    last_handshake: Option<Galahad2Handshake>,
}

impl Galahad2TrinityController {
    pub fn new(device: HidDevice, pid: u16) -> Result<Self> {
        let model = Galahad2TrinityModel::from_pid(pid)
            .ok_or_else(|| anyhow::anyhow!("Unknown Galahad2 Trinity PID: {pid:#06x}"))?;

        let mut ctrl = Self {
            device: Mutex::new(device),
            model,
            last_handshake: None,
        };

        ctrl.initialize()?;
        Ok(ctrl)
    }

    fn initialize(&mut self) -> Result<()> {
        info!("Initializing {} (PID={:#06x})", self.model.name(), match self.model {
            Galahad2TrinityModel::Performance => 0x7371u16,
            Galahad2TrinityModel::Regular => 0x7373u16,
        });

        match self.read_firmware() {
            Ok(fw) => info!("  Firmware: {fw}"),
            Err(e) => warn!("  Failed to read firmware: {e}"),
        }

        match self.handshake() {
            Ok(hs) => {
                info!("  Fan RPM: {}, Pump RPM: {}", hs.fan_rpm, hs.pump_rpm);
            }
            Err(e) => warn!("  Handshake failed: {e}"),
        }

        Ok(())
    }

    /// Perform handshake to read fan and pump RPM.
    pub fn handshake(&mut self) -> Result<Galahad2Handshake> {
        let resp = self.send_a_command(CMD_HANDSHAKE, &[])?;
        let data = &resp[HEADER_LEN..];
        let data_len = resp[5] as usize;

        if data_len < 4 {
            bail!("Galahad2 Trinity: handshake response too short ({data_len} bytes)");
        }

        let hs = Galahad2Handshake {
            fan_rpm: u16::from_be_bytes([data[0], data[1]]),
            pump_rpm: u16::from_be_bytes([data[2], data[3]]),
        };

        debug!("Handshake: fan={}rpm pump={}rpm", hs.fan_rpm, hs.pump_rpm);
        self.last_handshake = Some(hs.clone());
        Ok(hs)
    }

    fn read_firmware(&self) -> Result<String> {
        let resp = self.send_a_command(CMD_GET_FIRMWARE, &[])?;
        let data_len = resp[5] as usize;
        let data = &resp[HEADER_LEN..HEADER_LEN + data_len.min(58)];
        Ok(String::from_utf8_lossy(data)
            .trim_end_matches('\0')
            .to_string())
    }

    pub fn model(&self) -> Galahad2TrinityModel {
        self.model
    }

    // -- Packet helpers --

    fn send_a_command(&self, cmd: u8, data: &[u8]) -> Result<Vec<u8>> {
        let mut pkt = [0u8; PACKET_SIZE];
        pkt[0] = REPORT_ID;
        pkt[1] = cmd;
        pkt[5] = data.len() as u8;
        let copy_len = data.len().min(58);
        pkt[HEADER_LEN..HEADER_LEN + copy_len].copy_from_slice(&data[..copy_len]);

        let dev = self.device.lock();
        dev.write(&pkt).context("Galahad2 Trinity: write")?;

        let mut buf = [0u8; PACKET_SIZE];
        let n = dev
            .read_timeout(&mut buf, READ_TIMEOUT_MS)
            .context("Galahad2 Trinity: read")?;

        if n == 0 {
            bail!("Galahad2 Trinity: no response to command {cmd:#04x}");
        }

        Ok(buf[..n].to_vec())
    }
}

impl FanDevice for Galahad2TrinityController {
    fn set_fan_speed(&self, _slot: u8, duty: u8) -> Result<()> {
        // Single fan channel, duty 0-100%
        let pwm = duty.min(100);
        self.send_a_command(CMD_SET_FAN_PWM, &[0x00, pwm])?;
        debug!("Set fan PWM to {pwm}%");
        Ok(())
    }

    fn set_fan_speeds(&self, duties: &[u8]) -> Result<()> {
        if let Some(&duty) = duties.first() {
            self.set_fan_speed(0, duty)?;
        }
        Ok(())
    }

    fn read_fan_rpm(&self) -> Result<Vec<u16>> {
        Ok(vec![self
            .last_handshake
            .as_ref()
            .map(|hs| hs.fan_rpm)
            .unwrap_or(0)])
    }

    fn fan_slot_count(&self) -> u8 {
        1
    }
}

impl AioDevice for Galahad2TrinityController {
    fn set_pump_speed(&self, duty: u8) -> Result<()> {
        let pwm = duty.min(100);
        self.send_a_command(CMD_SET_PUMP_PWM, &[0x00, pwm])?;
        debug!("Set pump PWM to {pwm}%");
        Ok(())
    }

    fn read_pump_rpm(&self) -> Result<u16> {
        Ok(self
            .last_handshake
            .as_ref()
            .map(|hs| hs.pump_rpm)
            .unwrap_or(0))
    }

    fn read_coolant_temp(&self) -> Result<f32> {
        // Trinity has NO coolant temperature sensor
        bail!("Galahad2 Trinity does not have a coolant temperature sensor")
    }
}
