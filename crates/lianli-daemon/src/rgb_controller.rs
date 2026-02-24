//! RGB controller: manages LED effects for all RGB-capable devices.
//!
//! Coordinates between native config effects and OpenRGB overrides.
//! Wired devices use the `RgbDevice` trait. Wireless devices stream
//! compressed per-LED frames via the `WirelessController`.

use lianli_devices::traits::RgbDevice;
use lianli_devices::wireless::WirelessController;
use lianli_shared::rgb::{RgbAppConfig, RgbDeviceCapabilities, RgbEffect, RgbMode, RgbZoneInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Tracks a wireless device's RGB state for `send_rgb_direct`.
struct WirelessRgbState {
    mac: [u8; 6],
    fan_count: u8,
    leds_per_fan: u8,
    /// Monotonically increasing effect index (4 bytes, sent in RF header).
    effect_counter: u32,
}

pub struct RgbController {
    /// Wired RGB devices keyed by device_id.
    wired: HashMap<String, Box<dyn RgbDevice>>,
    /// Wireless controller for RF-based LED control.
    wireless: Option<Arc<WirelessController>>,
    /// Wireless device state keyed by device_id ("wireless:xx:xx:xx:xx:xx:xx").
    wireless_state: HashMap<String, WirelessRgbState>,
    /// Current RGB config (from AppConfig).
    config: Option<RgbAppConfig>,
    /// When true, OpenRGB has active control — suppress native config application.
    openrgb_active: bool,
}

impl RgbController {
    pub fn new(
        wired: HashMap<String, Box<dyn RgbDevice>>,
        wireless: Option<Arc<WirelessController>>,
    ) -> Self {
        let mut wireless_state = HashMap::new();

        // Build wireless state from discovered devices
        if let Some(ref w) = wireless {
            for dev in w.devices() {
                let device_id = format!("wireless:{}", dev.mac_str());
                wireless_state.insert(
                    device_id,
                    WirelessRgbState {
                        mac: dev.mac,
                        fan_count: dev.fan_count,
                        leds_per_fan: dev.fan_type.leds_per_fan(),
                        effect_counter: 0,
                    },
                );
            }
        }

        info!(
            "RGB controller: {} wired device(s), {} wireless device(s)",
            wired.len(),
            wireless_state.len()
        );

        Self {
            wired,
            wireless,
            wireless_state,
            config: None,
            openrgb_active: false,
        }
    }

    /// Apply an RGB config. Called on config load/change.
    pub fn apply_config(&mut self, config: &RgbAppConfig) {
        self.config = Some(config.clone());

        if !config.enabled {
            info!("RGB control disabled in config");
            return;
        }

        if self.openrgb_active {
            debug!("Skipping native RGB config — OpenRGB has active control");
            return;
        }

        for dev_cfg in &config.devices {
            for zone_cfg in &dev_cfg.zones {
                if let Err(e) =
                    self.set_effect(&dev_cfg.device_id, zone_cfg.zone_index, &zone_cfg.effect)
                {
                    warn!(
                        "Failed to apply RGB effect to {} zone {}: {e}",
                        dev_cfg.device_id, zone_cfg.zone_index
                    );
                }
            }
        }
    }

    /// Set an effect on a specific device zone.
    pub fn set_effect(
        &mut self,
        device_id: &str,
        zone: u8,
        effect: &RgbEffect,
    ) -> anyhow::Result<()> {
        // Try wired first
        if let Some(dev) = self.wired.get(device_id) {
            dev.set_zone_effect(zone, effect)?;
            debug!("Set RGB effect on {device_id} zone {zone}: {:?}", effect.mode);
            return Ok(());
        }

        // Try wireless
        if let (Some(ref wireless), Some(state)) =
            (&self.wireless, self.wireless_state.get_mut(device_id))
        {
            let total_leds = state.fan_count as usize * state.leds_per_fan as usize;
            let colors = render_solid_colors(effect, total_leds);

            state.effect_counter = state.effect_counter.wrapping_add(1);
            let idx = state.effect_counter.to_be_bytes();

            wireless.send_rgb_direct(&state.mac, &colors, &idx)?;
            debug!(
                "Set wireless RGB on {device_id}: {:?}, {} LEDs",
                effect.mode, total_leds
            );
            return Ok(());
        }

        anyhow::bail!("RGB device not found: {device_id}");
    }

    /// Set per-LED colors directly (used by OpenRGB `UpdateLEDs`).
    pub fn set_direct_colors(
        &mut self,
        device_id: &str,
        zone: u8,
        colors: &[[u8; 3]],
    ) -> anyhow::Result<()> {
        // Try wired
        if let Some(dev) = self.wired.get(device_id) {
            dev.set_direct_colors(zone, colors)?;
            return Ok(());
        }

        // Try wireless — direct color is the native mode
        if let (Some(ref wireless), Some(state)) =
            (&self.wireless, self.wireless_state.get_mut(device_id))
        {
            state.effect_counter = state.effect_counter.wrapping_add(1);
            let idx = state.effect_counter.to_be_bytes();
            wireless.send_rgb_direct(&state.mac, colors, &idx)?;
            return Ok(());
        }

        anyhow::bail!("RGB device not found: {device_id}");
    }

    /// Return RGB capabilities for all connected devices.
    pub fn capabilities(&self) -> Vec<RgbDeviceCapabilities> {
        let mut caps = Vec::new();

        // Wired devices
        for (device_id, dev) in &self.wired {
            caps.push(RgbDeviceCapabilities {
                device_id: device_id.clone(),
                device_name: device_id.clone(),
                supported_modes: dev.supported_modes(),
                zones: dev.zone_info(),
                supports_direct: dev.supports_direct(),
                total_led_count: dev.total_led_count(),
            });
        }

        // Wireless devices
        for (device_id, state) in &self.wireless_state {
            let total_leds =
                state.fan_count as u16 * state.leds_per_fan as u16;
            let zones: Vec<RgbZoneInfo> = (0..state.fan_count)
                .map(|i| RgbZoneInfo {
                    name: format!("Fan {}", i + 1),
                    led_count: state.leds_per_fan as u16,
                })
                .collect();

            caps.push(RgbDeviceCapabilities {
                device_id: device_id.clone(),
                device_name: device_id.clone(),
                supported_modes: vec![RgbMode::Static, RgbMode::Direct],
                zones,
                supports_direct: true,
                total_led_count: total_leds,
            });
        }

        caps
    }

    /// Called when OpenRGB connects — suppress native config.
    pub fn set_openrgb_active(&mut self, active: bool) {
        if self.openrgb_active != active {
            self.openrgb_active = active;
            if active {
                info!("OpenRGB took control — suppressing native RGB config");
            } else {
                info!("OpenRGB released control — restoring native RGB config");
                if let Some(config) = self.config.clone() {
                    self.apply_config(&config);
                }
            }
        }
    }

    /// Refresh wireless device list (call after rediscovery).
    pub fn refresh_wireless_devices(&mut self) {
        if let Some(ref w) = self.wireless {
            let mut new_state = HashMap::new();
            for dev in w.devices() {
                let device_id = format!("wireless:{}", dev.mac_str());
                // Preserve existing effect counter if device was already known
                let counter = self
                    .wireless_state
                    .get(&device_id)
                    .map(|s| s.effect_counter)
                    .unwrap_or(0);
                new_state.insert(
                    device_id,
                    WirelessRgbState {
                        mac: dev.mac,
                        fan_count: dev.fan_count,
                        leds_per_fan: dev.fan_type.leds_per_fan(),
                        effect_counter: counter,
                    },
                );
            }
            self.wireless_state = new_state;
        }
    }
}

/// Render a solid color array from an RgbEffect for the given LED count.
/// For Static mode, all LEDs get the first color. For Off, all LEDs are black.
fn render_solid_colors(effect: &RgbEffect, total_leds: usize) -> Vec<[u8; 3]> {
    let color = match effect.mode {
        RgbMode::Off => [0, 0, 0],
        _ => {
            let base = effect.colors.first().copied().unwrap_or([255, 255, 255]);
            // Apply brightness scaling (0-4 scale, 4 = full brightness)
            let scale = (effect.brightness as f32 / 4.0).clamp(0.0, 1.0);
            [
                (base[0] as f32 * scale) as u8,
                (base[1] as f32 * scale) as u8,
                (base[2] as f32 * scale) as u8,
            ]
        }
    };
    vec![color; total_leds]
}
