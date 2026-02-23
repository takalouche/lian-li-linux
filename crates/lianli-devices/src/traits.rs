use anyhow::Result;
use lianli_shared::screen::ScreenInfo;

/// A device that can control fan speeds.
pub trait FanDevice: Send + Sync {
    fn set_fan_speed(&self, slot: u8, duty: u8) -> Result<()>;
    fn set_fan_speeds(&self, duties: &[u8]) -> Result<()>;
    fn read_fan_rpm(&self) -> Result<Vec<u16>>;
    fn fan_slot_count(&self) -> u8;

    /// Per-port fan counts: `(port_index, fan_count)`.
    /// Default: single port with `fan_slot_count()` fans.
    fn fan_port_info(&self) -> Vec<(u8, u8)> {
        vec![(0, self.fan_slot_count())]
    }

    /// Whether individual fans can be set to different speeds.
    /// Default: true (per-fan). Override to false for per-port devices.
    fn per_fan_control(&self) -> bool {
        true
    }

    /// Whether this device supports motherboard RPM sync (hardware passthrough).
    fn supports_mb_sync(&self) -> bool {
        false
    }

    /// Enable or disable motherboard RPM sync for a port.
    /// Only meaningful for devices where `supports_mb_sync()` returns true.
    fn set_mb_rpm_sync(&self, _port: u8, _sync: bool) -> Result<()> {
        anyhow::bail!("MB RPM sync not supported by this device")
    }
}

/// A device with an LCD screen.
pub trait LcdDevice: Send + Sync {
    fn screen_info(&self) -> &ScreenInfo;
    fn send_jpeg_frame(&mut self, jpeg_data: &[u8]) -> Result<()>;
    fn set_brightness(&self, brightness: u8) -> Result<()>;
    fn set_rotation(&self, degrees: u16) -> Result<()>;
    fn initialize(&mut self) -> Result<()>;
}

/// An AIO device with pump, fans, and optionally LCD.
pub trait AioDevice: FanDevice {
    fn set_pump_speed(&self, duty: u8) -> Result<()>;
    fn read_pump_rpm(&self) -> Result<u16>;
    fn read_coolant_temp(&self) -> Result<f32>;
}
