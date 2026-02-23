//! TL Fan controller driver.
//!
//! VID=0x0416, PID=0x7372
//!
//! Protocol uses HID Output Reports with Report ID 0x01.
//! 64-byte packets with a 6-byte header: [reportId, cmd, reserved, pktNumHi, pktNumLo, dataLen].
//! Each command expects a synchronous response (read after write).
//!
//! The controller supports 4 ports, each with multiple fans.
//! Fan speed is set per-fan via command 0xAA.
//! RPM values are only available from the handshake response (0xA1).

use crate::traits::FanDevice;
use anyhow::{bail, Context, Result};
use hidapi::HidDevice;
use parking_lot::Mutex;
use tracing::{debug, info, warn};

const REPORT_ID: u8 = 0x01;
const PACKET_SIZE: usize = 64;
const HEADER_LEN: usize = 6;
const MAX_PAYLOAD: usize = PACKET_SIZE - HEADER_LEN;
const READ_TIMEOUT_MS: i32 = 100;

// Commands
const CMD_HANDSHAKE: u8 = 0xA1;
const CMD_GET_PRODUCT_INFO: u8 = 0xA6;
const CMD_SET_FAN_SPEED: u8 = 0xAA;
const CMD_SET_MB_RPM_SYNC: u8 = 0xB1;

/// Information about a single detected fan.
#[derive(Debug, Clone)]
pub struct TlFanInfo {
    pub port: u8,
    pub fan_index: u8,
    pub rpm: u16,
    pub is_detected: bool,
}

/// TL Fan handshake result containing discovered fans per port.
#[derive(Debug, Clone)]
pub struct TlFanHandshake {
    /// Fans detected on each port. Index = port number (0-3).
    pub port_fan_counts: [u8; 4],
    /// All detected fans with their RPM values.
    pub fans: Vec<TlFanInfo>,
}

/// TL Fan controller.
///
/// Wraps an opened HID device for a TL Fan controller (0x0416:0x7372).
/// Provides fan speed control and RPM reading via the handshake protocol.
/// Does NOT touch RGB/LED effects — that's OpenRGB's domain.
pub struct TlFanController {
    device: Mutex<HidDevice>,
    /// Last handshake result (updated on each handshake call).
    /// Behind a Mutex for interior mutability — allows `read_fan_rpm(&self)`
    /// to refresh RPMs while the device is shared across threads.
    last_handshake: Mutex<Option<TlFanHandshake>>,
}

impl TlFanController {
    /// Open a TL Fan controller from an already-opened HID device.
    pub fn new(device: HidDevice) -> Result<Self> {
        let ctrl = Self {
            device: Mutex::new(device),
            last_handshake: Mutex::new(None),
        };

        ctrl.initialize()?;
        Ok(ctrl)
    }

    /// Initialize: perform handshake to discover fans.
    fn initialize(&self) -> Result<()> {
        info!("Initializing TL Fan Controller (0x0416:0x7372)");

        match self.read_product_info() {
            Ok(version) => info!("  Firmware: {version}"),
            Err(e) => warn!("  Failed to read firmware: {e}"),
        }

        match self.handshake() {
            Ok(hs) => {
                info!(
                    "  Detected fans: port0={}, port1={}, port2={}, port3={}",
                    hs.port_fan_counts[0],
                    hs.port_fan_counts[1],
                    hs.port_fan_counts[2],
                    hs.port_fan_counts[3]
                );
                for fan in &hs.fans {
                    debug!(
                        "    Port {} Fan {}: {} RPM",
                        fan.port, fan.fan_index, fan.rpm
                    );
                }
            }
            Err(e) => warn!("  Handshake failed: {e}"),
        }

        Ok(())
    }

    /// Perform a handshake to discover connected fans and read RPMs.
    pub fn handshake(&self) -> Result<TlFanHandshake> {
        let response = self.send_command(CMD_HANDSHAKE, &[])?;

        let mut port_fan_counts = [0u8; 4];
        let mut fans = Vec::new();

        // Response data: 3 bytes per fan entry
        // Byte 0: [7]IsDetected | [6]IsUpgrading | [5:4]Port | [3:0]FanIndex
        // Byte 1: RPM high byte
        // Byte 2: RPM low byte
        let data = &response[HEADER_LEN..];
        let data_len = response[5] as usize;
        let fan_count = data_len / 3;

        for i in 0..fan_count {
            let offset = i * 3;
            if offset + 2 >= data.len() {
                break;
            }

            let info_byte = data[offset];
            let is_detected = (info_byte & 0x80) != 0;
            let port = (info_byte >> 4) & 0x03;
            let fan_index = info_byte & 0x0F;
            let rpm = u16::from_be_bytes([data[offset + 1], data[offset + 2]]);

            if is_detected {
                port_fan_counts[port as usize] =
                    port_fan_counts[port as usize].max(fan_index + 1);
            }

            fans.push(TlFanInfo {
                port,
                fan_index,
                rpm,
                is_detected,
            });
        }

        let hs = TlFanHandshake {
            port_fan_counts,
            fans,
        };
        *self.last_handshake.lock() = Some(hs.clone());
        Ok(hs)
    }

    /// Read product/firmware info.
    fn read_product_info(&self) -> Result<String> {
        // Request controller firmware (not fan firmware)
        let response = self.send_command(CMD_GET_PRODUCT_INFO, &[0x00, 0x00])?;
        let data_len = response[5] as usize;
        let data = &response[HEADER_LEN..HEADER_LEN + data_len.min(MAX_PAYLOAD)];

        // Firmware version is ASCII text
        let version = String::from_utf8_lossy(data)
            .trim_end_matches('\0')
            .to_string();
        Ok(version)
    }

    /// Set fan speed (PWM duty) for a specific port and fan index.
    ///
    /// `port`: 0-3
    /// `fan_index`: 0-15 (typically 0-3)
    /// `duty`: 0-255 (0% to 100%)
    pub fn set_fan_speed_single(&self, port: u8, fan_index: u8, duty: u8) -> Result<()> {
        if port >= 4 {
            bail!("Port {port} out of range (0-3)");
        }

        let addr = (port << 4) | (fan_index & 0x0F);
        self.send_command_quiet(CMD_SET_FAN_SPEED, &[addr, duty])?;

        debug!(
            "Set port {} fan {} speed to duty={duty} ({:.0}%)",
            port,
            fan_index,
            duty as f32 / 2.55
        );
        Ok(())
    }

    /// Set the same speed for all fans on a port.
    pub fn set_port_speed(&self, port: u8, duty: u8) -> Result<()> {
        let fan_count = self
            .last_handshake
            .lock()
            .as_ref()
            .map(|hs| hs.port_fan_counts[port as usize])
            .unwrap_or(1);

        for idx in 0..fan_count {
            self.set_fan_speed_single(port, idx, duty)?;
        }
        Ok(())
    }

    /// Enable or disable motherboard RPM sync for a specific fan.
    ///
    /// When enabled, the TL Fan Controller firmware reads the motherboard's
    /// PWM signal directly from its 4-pin fan header, bypassing software control.
    ///
    /// Data byte: `(isSync << 7) | (port << 4) | fanIndex`
    pub fn set_mb_rpm_sync(&self, port: u8, fan_index: u8, sync: bool) -> Result<()> {
        if port >= 4 {
            bail!("Port {port} out of range (0-3)");
        }
        let data = ((sync as u8) << 7) | (port << 4) | (fan_index & 0x0F);
        self.send_command_quiet(CMD_SET_MB_RPM_SYNC, &[data])?;
        debug!(
            "Set MB RPM sync port={port} fan={fan_index} sync={sync}"
        );
        Ok(())
    }

    /// Enable or disable motherboard RPM sync for all fans on a port.
    pub fn set_port_mb_rpm_sync(&self, port: u8, sync: bool) -> Result<()> {
        let fan_count = self
            .last_handshake
            .lock()
            .as_ref()
            .map(|hs| hs.port_fan_counts[port as usize])
            .unwrap_or(1);

        for idx in 0..fan_count {
            self.set_mb_rpm_sync(port, idx, sync)?;
        }
        Ok(())
    }

    /// Total number of detected fans across all ports.
    pub fn total_fan_count(&self) -> u8 {
        self.last_handshake
            .lock()
            .as_ref()
            .map(|hs| hs.port_fan_counts.iter().sum())
            .unwrap_or(0)
    }

    // -- Low-level packet helpers --

    /// Build a TL Fan packet.
    fn build_packet(cmd: u8, data: &[u8]) -> [u8; PACKET_SIZE] {
        let mut pkt = [0u8; PACKET_SIZE];
        pkt[0] = REPORT_ID;
        pkt[1] = cmd;
        pkt[2] = 0x00; // reserved
        pkt[3] = 0x00; // packet number high
        pkt[4] = 0x00; // packet number low
        pkt[5] = data.len() as u8;

        let copy_len = data.len().min(MAX_PAYLOAD);
        pkt[HEADER_LEN..HEADER_LEN + copy_len].copy_from_slice(&data[..copy_len]);
        pkt
    }

    /// Drain any stale data sitting in the read buffer.
    fn drain_read_buffer(dev: &HidDevice) {
        let mut buf = [0u8; PACKET_SIZE];
        while dev.read_timeout(&mut buf, 0).unwrap_or(0) > 0 {}
    }

    /// Send a command, try to read a response but ignore failure.
    /// Matches L-Connect 3 behavior where readPacket() result is discarded.
    fn send_command_quiet(&self, cmd: u8, data: &[u8]) -> Result<()> {
        let pkt = Self::build_packet(cmd, data);
        let dev = self.device.lock();
        Self::drain_read_buffer(&dev);
        dev.write(&pkt).context("TL Fan: write command")?;
        let mut buf = [0u8; PACKET_SIZE];
        let _ = dev.read_timeout(&mut buf, READ_TIMEOUT_MS);
        Ok(())
    }

    /// Send a command and read the synchronous response.
    fn send_command(&self, cmd: u8, data: &[u8]) -> Result<Vec<u8>> {
        let pkt = Self::build_packet(cmd, data);
        let dev = self.device.lock();

        Self::drain_read_buffer(&dev);

        dev.write(&pkt)
            .context("TL Fan: write command")?;

        let mut buf = [0u8; PACKET_SIZE];
        // Read up to a few times to skip stale responses
        for _ in 0..5 {
            let n = dev
                .read_timeout(&mut buf, READ_TIMEOUT_MS)
                .context("TL Fan: read response")?;

            if n == 0 {
                bail!("TL Fan: no response to command {cmd:#04x}");
            }

            if buf[1] == cmd {
                return Ok(buf[..n].to_vec());
            }

            debug!(
                "TL Fan: skipping stale response {:#04x} (waiting for {cmd:#04x})",
                buf[1]
            );
        }

        bail!("TL Fan: never received response for command {cmd:#04x}");
    }
}

impl FanDevice for TlFanController {
    fn set_fan_speed(&self, slot: u8, duty: u8) -> Result<()> {
        // Treat slot as port index, set all fans on that port
        self.set_port_speed(slot, duty)
    }

    fn set_fan_speeds(&self, duties: &[u8]) -> Result<()> {
        // Set speed per port
        for (port, &duty) in duties.iter().take(4).enumerate() {
            self.set_port_speed(port as u8, duty)?;
        }
        Ok(())
    }

    fn read_fan_rpm(&self) -> Result<Vec<u16>> {
        // Refresh handshake for live RPM data
        let _ = self.handshake();

        let guard = self.last_handshake.lock();
        match guard.as_ref() {
            Some(hs) => {
                // Return per-fan RPMs ordered by port then fan_index
                let mut rpms = Vec::new();
                for port in 0..4u8 {
                    for fan_idx in 0..hs.port_fan_counts[port as usize] {
                        let rpm = hs
                            .fans
                            .iter()
                            .find(|f| f.port == port && f.fan_index == fan_idx && f.is_detected)
                            .map(|f| f.rpm)
                            .unwrap_or(0);
                        rpms.push(rpm);
                    }
                }
                Ok(rpms)
            }
            None => Ok(vec![]),
        }
    }

    fn fan_slot_count(&self) -> u8 {
        4 // 4 ports
    }

    fn fan_port_info(&self) -> Vec<(u8, u8)> {
        let guard = self.last_handshake.lock();
        match guard.as_ref() {
            Some(hs) => hs
                .port_fan_counts
                .iter()
                .enumerate()
                .filter(|(_, &count)| count > 0)
                .map(|(port, &count)| (port as u8, count))
                .collect(),
            None => vec![(0, 4)],
        }
    }

    fn per_fan_control(&self) -> bool {
        false // all fans on a port share the same speed
    }

    fn supports_mb_sync(&self) -> bool {
        true
    }

    fn set_mb_rpm_sync(&self, port: u8, sync: bool) -> Result<()> {
        self.set_port_mb_rpm_sync(port, sync)
    }
}
