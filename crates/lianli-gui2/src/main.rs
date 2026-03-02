mod backend;
mod conversions;
mod ipc_client;
mod state;

use lianli_shared::fan::{FanConfig, FanCurve, FanGroup, FanSpeed};
use lianli_shared::ipc::IpcRequest;
use lianli_shared::rgb::{
    RgbAppConfig, RgbDeviceConfig, RgbDirection, RgbEffect, RgbMode, RgbScope, RgbZoneConfig,
};
use slint::Model;
use std::sync::{Arc, Mutex};

slint::include_modules!();

/// Shared mutable state: config + cached capabilities + devices.
/// Backend thread updates it on load; callbacks mutate config; save sends it.
pub type Shared = Arc<Mutex<state::SharedState>>;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("lianli_gui2=info".parse().unwrap()),
        )
        .init();

    let window = MainWindow::new().expect("Failed to create main window");

    // Shared state — backend will populate on first load
    let shared: Shared = Arc::new(Mutex::new(state::SharedState::default()));
    let backend = backend::start(window.as_weak(), shared.clone());

    // ── Refresh devices ──
    {
        let tx = backend.tx.clone();
        window.on_refresh_devices(move || {
            let _ = tx.send(backend::BackendCommand::RefreshDevices);
        });
    }

    // ── Save config ──
    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        window.on_save_config(move || {
            let state = shared.lock().unwrap();
            if let Some(ref c) = state.config {
                let _ = tx.send(backend::BackendCommand::SaveConfig(c.clone()));
            }
        });
    }

    // ── Toggle OpenRGB ──
    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        window.on_toggle_openrgb(move |enabled| {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let rgb = c.rgb.get_or_insert_with(Default::default);
                    rgb.openrgb_server = enabled;
                    let _ = tx.send(backend::BackendCommand::SaveConfig(c.clone()));
                }
            }
        });
    }

    // ── RGB callbacks ──
    wire_rgb_callbacks(&window, &backend, &shared);

    // ── Fan callbacks ──
    wire_fan_callbacks(&window, &backend, &shared);

    // ── LCD callbacks ──
    wire_lcd_callbacks(&window, &shared);

    window.run().expect("Failed to run Slint event loop");
    backend.send(backend::BackendCommand::Shutdown);
}

fn wire_rgb_callbacks(
    window: &MainWindow,
    backend: &backend::BackendHandle,
    shared: &Shared,
) {
    // RGB set mode
    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_set_mode(move |dev_id, zone, mode| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let mode_enum = parse_rgb_mode(&mode);

            let effect = with_zone_effect(&shared, &dev_id, zone, |e| {
                e.mode = mode_enum;
            });

            send_rgb_effect(&tx, &shared, &dev_id, zone, &effect);
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_set_speed(move |dev_id, zone, speed| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let effect = with_zone_effect(&shared, &dev_id, zone, |e| {
                e.speed = speed as u8;
            });
            send_rgb_effect(&tx, &shared, &dev_id, zone, &effect);
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_set_brightness(move |dev_id, zone, brightness| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let effect = with_zone_effect(&shared, &dev_id, zone, |e| {
                e.brightness = brightness as u8;
            });
            send_rgb_effect(&tx, &shared, &dev_id, zone, &effect);
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_set_direction(move |dev_id, zone, dir| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let effect = with_zone_effect(&shared, &dev_id, zone, |e| {
                e.direction = parse_rgb_direction(&dir);
            });
            send_rgb_effect(&tx, &shared, &dev_id, zone, &effect);
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_set_scope(move |dev_id, zone, scope| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let effect = with_zone_effect(&shared, &dev_id, zone, |e| {
                e.scope = parse_rgb_scope(&scope);
            });
            send_rgb_effect(&tx, &shared, &dev_id, zone, &effect);
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_set_color(move |dev_id, zone, cidx, r, g, b| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let effect = with_zone_effect(&shared, &dev_id, zone, |e| {
                let cidx = cidx as usize;
                while e.colors.len() <= cidx {
                    e.colors.push([255, 255, 255]);
                }
                e.colors[cidx] = [r as u8, g as u8, b as u8];
            });
            send_rgb_effect(&tx, &shared, &dev_id, zone, &effect);
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_toggle_mb_sync(move |dev_id, enabled| {
            let dev_id = dev_id.to_string();
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let rgb = c.rgb.get_or_insert_with(Default::default);
                    let base_id = dev_id.split(":port").next().unwrap_or(&dev_id);
                    for dev_cfg in &mut rgb.devices {
                        if dev_cfg.device_id.starts_with(base_id) {
                            dev_cfg.mb_rgb_sync = enabled;
                        }
                    }
                    if !rgb.devices.iter().any(|d| d.device_id == dev_id) {
                        rgb.devices.push(RgbDeviceConfig {
                            device_id: dev_id.clone(),
                            mb_rgb_sync: enabled,
                            zones: vec![],
                        });
                    }
                }
            }
            let _ = tx.send(backend::BackendCommand::IpcRequest(
                IpcRequest::SetMbRgbSync {
                    device_id: dev_id,
                    enabled,
                },
            ));
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        window.on_rgb_apply_to_all(move |dev_id| {
            let dev_id = dev_id.to_string();
            let state = shared.lock().unwrap();
            if let Some(ref c) = state.config {
                if let Some(rgb) = &c.rgb {
                    if let Some(dev_cfg) = rgb.devices.iter().find(|d| d.device_id == dev_id) {
                        if let Some(z0) = dev_cfg.zones.first() {
                            let effect = z0.effect.clone();
                            for zone_cfg in &dev_cfg.zones {
                                let _ = tx.send(backend::BackendCommand::IpcRequest(
                                    IpcRequest::SetRgbEffect {
                                        device_id: dev_id.clone(),
                                        zone: zone_cfg.zone_index,
                                        effect: effect.clone(),
                                    },
                                ));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_toggle_swap_lr(move |dev_id, zone| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let (swap_lr, swap_tb) = {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let rgb = c.rgb.get_or_insert_with(Default::default);
                    let dev = get_or_create_device_config(rgb, &dev_id);
                    let zcfg = get_or_create_zone_config(dev, zone);
                    zcfg.swap_lr = !zcfg.swap_lr;
                    (zcfg.swap_lr, zcfg.swap_tb)
                } else {
                    return;
                }
            };
            let _ = tx.send(backend::BackendCommand::IpcRequest(
                IpcRequest::SetFanDirection {
                    device_id: dev_id,
                    zone,
                    swap_lr,
                    swap_tb,
                },
            ));
            refresh_rgb_ui(&weak, &shared);
        });
    }

    {
        let tx = backend.tx.clone();
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rgb_toggle_swap_tb(move |dev_id, zone| {
            let dev_id = dev_id.to_string();
            let zone = zone as u8;
            let (swap_lr, swap_tb) = {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let rgb = c.rgb.get_or_insert_with(Default::default);
                    let dev = get_or_create_device_config(rgb, &dev_id);
                    let zcfg = get_or_create_zone_config(dev, zone);
                    zcfg.swap_tb = !zcfg.swap_tb;
                    (zcfg.swap_lr, zcfg.swap_tb)
                } else {
                    return;
                }
            };
            let _ = tx.send(backend::BackendCommand::IpcRequest(
                IpcRequest::SetFanDirection {
                    device_id: dev_id,
                    zone,
                    swap_lr,
                    swap_tb,
                },
            ));
            refresh_rgb_ui(&weak, &shared);
        });
    }
}

fn wire_fan_callbacks(
    window: &MainWindow,
    _backend: &backend::BackendHandle,
    shared: &Shared,
) {
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_add_curve(move || {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let n = c.fan_curves.len() + 1;
                    c.fan_curves.push(FanCurve {
                        name: format!("curve-{n}"),
                        temp_command: "cat /sys/class/thermal/thermal_zone0/temp | awk '{print $1/1000}'"
                            .to_string(),
                        curve: vec![(30.0, 30.0), (50.0, 50.0), (70.0, 80.0), (85.0, 100.0)],
                    });
                }
            }
            refresh_fan_ui(&weak, &shared);
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_remove_curve(move |idx| {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let idx = idx as usize;
                    if idx < c.fan_curves.len() {
                        c.fan_curves.remove(idx);
                    }
                }
            }
            refresh_fan_ui(&weak, &shared);
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_rename_curve(move |idx, name| {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    if let Some(curve) = c.fan_curves.get_mut(idx as usize) {
                        curve.name = name.to_string();
                    }
                }
            }
            // Don't rebuild model — would destroy the focused LineEdit.
            // The typed text is already visible. Mark dirty only.
            if let Some(w) = weak.upgrade() {
                w.set_config_dirty(true);
            }
        });
    }

    {
        let shared = shared.clone();
        window.on_fan_set_temp_command(move |idx, cmd| {
            let mut state = shared.lock().unwrap();
            if let Some(ref mut c) = state.config {
                if let Some(curve) = c.fan_curves.get_mut(idx as usize) {
                    curve.temp_command = cmd.to_string();
                }
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_point_moved(move |cidx, pidx, temp, speed| {
            let temp = temp.round().clamp(20.0, 100.0) as f32;
            let speed = speed.round().clamp(0.0, 100.0) as f32;
            let cidx_u = cidx as usize;
            let pidx_u = pidx as usize;

            // Update shared state, get sorted points for path rebuild
            let sorted = {
                let mut state = shared.lock().unwrap();
                let c = match state.config.as_mut() {
                    Some(c) => c,
                    None => return,
                };
                let curve = match c.fan_curves.get_mut(cidx_u) {
                    Some(curve) => curve,
                    None => return,
                };
                if let Some(pt) = curve.curve.get_mut(pidx_u) {
                    pt.0 = temp;
                    pt.1 = speed;
                }
                let mut sorted = curve.curve.clone();
                sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                sorted
            };

            // Synchronous in-place model update (we're on the UI thread).
            // This preserves the TouchArea so the drag continues.
            if let Some(w) = weak.upgrade() {
                let model = w.get_fan_curves();
                if let Some(mut curve_data) = model.row_data(cidx_u) {
                    // Update inner points model in-place
                    curve_data.points.set_row_data(pidx_u, CurvePoint { temp, speed });
                    // Update segment models
                    curve_data.curve_segments = slint::ModelRc::new(
                        slint::VecModel::from(conversions::build_curve_segments(&sorted)),
                    );
                    curve_data.clamp_segments = slint::ModelRc::new(
                        slint::VecModel::from(conversions::build_clamp_segments(&sorted)),
                    );
                    model.set_row_data(cidx_u, curve_data);
                    w.set_config_dirty(true);
                }
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_point_added(move |cidx, temp, speed| {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    if let Some(curve) = c.fan_curves.get_mut(cidx as usize) {
                        curve.curve.push((
                            temp.round().clamp(20.0, 100.0),
                            speed.round().clamp(0.0, 100.0),
                        ));
                    }
                }
            }
            refresh_fan_ui(&weak, &shared);
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_point_removed(move |cidx, pidx| {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    if let Some(curve) = c.fan_curves.get_mut(cidx as usize) {
                        let pidx = pidx as usize;
                        if pidx < curve.curve.len() {
                            curve.curve.remove(pidx);
                        }
                    }
                }
            }
            refresh_fan_ui(&weak, &shared);
        });
    }

    // Fan speed assignment
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_set_slot_speed(move |dev_id, slot, val| {
            let dev_id = dev_id.to_string();
            let slot = slot as usize;
            let val = val.to_string();
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let fc = c.fans.get_or_insert_with(|| FanConfig {
                        speeds: vec![],
                        update_interval_ms: 1000,
                    });
                    let group = fc.speeds.iter_mut().find(|g| g.device_id.as_deref() == Some(&dev_id));
                    let group = if let Some(g) = group {
                        g
                    } else {
                        fc.speeds.push(FanGroup {
                            device_id: Some(dev_id.clone()),
                            speeds: [FanSpeed::Constant(0), FanSpeed::Constant(0), FanSpeed::Constant(0), FanSpeed::Constant(0)],
                        });
                        fc.speeds.last_mut().unwrap()
                    };

                    let speed: FanSpeed = match val.as_str() {
                        "Off" => FanSpeed::Constant(0),
                        "Constant PWM" => FanSpeed::Constant(128),
                        "MB Sync" => FanSpeed::Curve("__mb_sync__".to_string()),
                        curve_name => FanSpeed::Curve(curve_name.to_string()),
                    };
                    if slot < 4 {
                        group.speeds[slot] = speed;
                    }
                }
            }
            refresh_fan_ui(&weak, &shared);
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_fan_set_slot_pwm(move |dev_id, slot, percent| {
            let dev_id = dev_id.to_string();
            let slot = slot as usize;
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    if let Some(fc) = &mut c.fans {
                        if let Some(group) = fc.speeds.iter_mut().find(|g| g.device_id.as_deref() == Some(&dev_id)) {
                            if slot < 4 {
                                group.speeds[slot] = FanSpeed::Constant(((percent as f32 / 100.0) * 255.0).round() as u8);
                            }
                        }
                    }
                }
            }
            // In-place update to avoid destroying the Slider during drag
            if let Some(w) = weak.upgrade() {
                let model = w.get_fan_groups();
                for i in 0..model.row_count() {
                    if let Some(group_data) = model.row_data(i) {
                        if group_data.device_id.as_str() == dev_id {
                            if let Some(mut slot_data) = group_data.slots.row_data(slot) {
                                slot_data.pwm_percent = percent;
                                group_data.slots.set_row_data(slot, slot_data);
                            }
                            break;
                        }
                    }
                }
                w.set_config_dirty(true);
            }
        });
    }
}

fn wire_lcd_callbacks(
    window: &MainWindow,
    shared: &Shared,
) {
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_add_lcd(move || {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    c.lcds.push(lianli_shared::config::LcdConfig {
                        index: None,
                        serial: None,
                        media_type: lianli_shared::media::MediaType::Image,
                        path: None,
                        fps: Some(30.0),
                        rgb: None,
                        orientation: 0.0,
                        sensor: None,
                    });
                }
            }
            refresh_lcd_ui(&weak, &shared);
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_remove_lcd(move |idx| {
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let idx = idx as usize;
                    if idx < c.lcds.len() {
                        c.lcds.remove(idx);
                    }
                }
            }
            refresh_lcd_ui(&weak, &shared);
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_update_lcd_field(move |idx, field, val| {
            let field_str = field.to_string();
            // Only rebuild UI for dropdown/button fields that affect layout.
            // Text fields update in-place in the LineEdit — rebuilding would steal focus.
            let needs_refresh = matches!(field_str.as_str(), "serial" | "media_type" | "orientation");
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let idx = idx as usize;
                    if let Some(lcd) = c.lcds.get_mut(idx) {
                        let val = val.to_string();
                        match field_str.as_str() {
                            "serial" => lcd.serial = Some(val),
                            "media_type" => {
                                lcd.media_type = match val.as_str() {
                                    "Image" => lianli_shared::media::MediaType::Image,
                                    "Video" => lianli_shared::media::MediaType::Video,
                                    "GIF" => lianli_shared::media::MediaType::Gif,
                                    "Solid Color" => lianli_shared::media::MediaType::Color,
                                    "Sensor Gauge" => lianli_shared::media::MediaType::Sensor,
                                    _ => lcd.media_type,
                                };
                            }
                            "path" => lcd.path = Some(std::path::PathBuf::from(val)),
                            "orientation" => lcd.orientation = val.parse().unwrap_or(0.0),
                            "sensor_label" => {
                                lcd.sensor.get_or_insert_with(default_sensor).label = val;
                            }
                            "sensor_unit" => {
                                lcd.sensor.get_or_insert_with(default_sensor).unit = val;
                            }
                            "sensor_command" => {
                                lcd.sensor.get_or_insert_with(default_sensor).source =
                                    lianli_shared::media::SensorSourceConfig::Command { cmd: val };
                            }
                            "sensor_font_path" => {
                                lcd.sensor.get_or_insert_with(default_sensor).font_path =
                                    Some(std::path::PathBuf::from(val));
                            }
                            _ => {}
                        }
                    }
                }
            }
            if needs_refresh {
                refresh_lcd_ui(&weak, &shared);
            } else if let Some(w) = weak.upgrade() {
                w.set_config_dirty(true);
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_pick_lcd_file(move |idx| {
            let shared2 = shared.clone();
            let weak2 = weak.clone();
            let idx = idx as usize;
            std::thread::spawn(move || {
                let file = rfd::FileDialog::new()
                    .add_filter(
                        "Media",
                        &["jpg", "jpeg", "png", "bmp", "gif", "mp4", "avi", "mkv", "webm"],
                    )
                    .pick_file();
                if let Some(path) = file {
                    {
                        let mut state = shared2.lock().unwrap();
                        if let Some(ref mut c) = state.config {
                            if let Some(lcd) = c.lcds.get_mut(idx) {
                                lcd.path = Some(path);
                            }
                        }
                    }
                    refresh_lcd_ui(&weak2, &shared2);
                }
            });
        });
    }
}

// ── Refresh helpers ──
// These read from SharedState (lock briefly), then push models to UI via invoke_from_event_loop.

fn refresh_fan_ui(weak: &slint::Weak<MainWindow>, shared: &Shared) {
    let (curves, fans, devices) = {
        let state = shared.lock().unwrap();
        let config = match state.config.as_ref() {
            Some(c) => c,
            None => return,
        };
        (config.fan_curves.clone(), config.fans.clone(), state.devices.clone())
    };

    let weak = weak.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(w) = weak.upgrade() {
            w.set_fan_curves(conversions::fan_curves_to_model(&curves));
            w.set_curve_names(conversions::curve_names_to_model(&curves));
            w.set_fan_speed_options(conversions::speed_options_model(&curves, true));
            w.set_config_dirty(true);
            if let Some(ref fc) = fans {
                w.set_fan_groups(conversions::fan_groups_to_model(fc, &devices));
            }
        }
    })
    .ok();
}

fn refresh_lcd_ui(weak: &slint::Weak<MainWindow>, shared: &Shared) {
    let lcds = {
        let state = shared.lock().unwrap();
        match state.config.as_ref() {
            Some(c) => c.lcds.clone(),
            None => return,
        }
    };

    let weak = weak.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(w) = weak.upgrade() {
            w.set_lcd_entries(conversions::lcd_entries_to_model(&lcds));
            w.set_config_dirty(true);
        }
    })
    .ok();
}

fn refresh_rgb_ui(weak: &slint::Weak<MainWindow>, shared: &Shared) {
    let (rgb_caps, config) = {
        let state = shared.lock().unwrap();
        match state.config.as_ref() {
            Some(c) => (state.rgb_caps.clone(), c.clone()),
            None => return,
        }
    };

    let weak = weak.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(w) = weak.upgrade() {
            let rgb_model = conversions::rgb_devices_to_model(&rgb_caps, &config);
            w.set_rgb_devices(rgb_model);
            w.set_config_dirty(true);
        }
    })
    .ok();
}

fn default_sensor() -> lianli_shared::media::SensorDescriptor {
    lianli_shared::media::SensorDescriptor {
        label: "CPU".to_string(),
        unit: "\u{00B0}C".to_string(),
        source: lianli_shared::media::SensorSourceConfig::Command {
            cmd: String::new(),
        },
        text_color: [255, 255, 255],
        background_color: [0, 0, 0],
        gauge_background_color: [40, 40, 40],
        gauge_ranges: vec![],
        update_interval_ms: 1000,
        gauge_start_angle: 135.0,
        gauge_sweep_angle: 270.0,
        gauge_outer_radius: 200.0,
        gauge_thickness: 30.0,
        bar_corner_radius: 5.0,
        value_font_size: 120.0,
        unit_font_size: 40.0,
        label_font_size: 30.0,
        font_path: None,
        decimal_places: 0,
        value_offset: 0,
        unit_offset: 0,
        label_offset: 0,
    }
}

/// Get or update an RGB zone's effect in the shared state, returning the updated effect.
fn with_zone_effect(
    shared: &Shared,
    dev_id: &str,
    zone: u8,
    mutate: impl FnOnce(&mut RgbEffect),
) -> RgbEffect {
    let mut state = shared.lock().unwrap();
    let c = match state.config.as_mut() {
        Some(c) => c,
        None => {
            let mut e = RgbEffect {
                mode: RgbMode::Static,
                colors: vec![[255, 255, 255]],
                speed: 2,
                brightness: 4,
                direction: RgbDirection::Clockwise,
                scope: RgbScope::All,
            };
            mutate(&mut e);
            return e;
        }
    };

    let rgb = c.rgb.get_or_insert_with(Default::default);
    let dev = get_or_create_device_config(rgb, dev_id);
    let zcfg = get_or_create_zone_config(dev, zone);
    mutate(&mut zcfg.effect);
    zcfg.effect.clone()
}

/// Check if a device has group zones (Top/Bottom scopes) and return zone count.
fn device_group_zone_count(shared: &Shared, dev_id: &str) -> Option<usize> {
    let state = shared.lock().unwrap();
    let cap = state.rgb_caps.iter().find(|c| c.device_id == dev_id)?;
    let has_group = cap.supported_scopes.iter().any(|scopes| {
        scopes.iter().any(|s| matches!(s, RgbScope::Top | RgbScope::Bottom))
    });
    if has_group { Some(cap.zones.len()) } else { None }
}

/// Send RGB effect IPC, broadcasting to all zones if zone 0 on a group-zone device.
fn send_rgb_effect(
    tx: &std::sync::mpsc::Sender<backend::BackendCommand>,
    shared: &Shared,
    dev_id: &str,
    zone: u8,
    effect: &RgbEffect,
) {
    let zones_to_update: Vec<u8> = if zone == 0 {
        if let Some(zone_count) = device_group_zone_count(shared, dev_id) {
            // Broadcast: update all zones in config with the same effect
            {
                let mut state = shared.lock().unwrap();
                if let Some(ref mut c) = state.config {
                    let rgb = c.rgb.get_or_insert_with(Default::default);
                    let dev = get_or_create_device_config(rgb, dev_id);
                    for z in 1..zone_count as u8 {
                        let zcfg = get_or_create_zone_config(dev, z);
                        zcfg.effect = effect.clone();
                    }
                }
            }
            (0..zone_count as u8).collect()
        } else {
            vec![zone]
        }
    } else {
        vec![zone]
    };

    for z in zones_to_update {
        let _ = tx.send(backend::BackendCommand::IpcRequest(
            IpcRequest::SetRgbEffect {
                device_id: dev_id.to_string(),
                zone: z,
                effect: effect.clone(),
            },
        ));
    }
}

fn get_or_create_device_config<'a>(
    rgb: &'a mut RgbAppConfig,
    dev_id: &str,
) -> &'a mut RgbDeviceConfig {
    if !rgb.devices.iter().any(|d| d.device_id == dev_id) {
        rgb.devices.push(RgbDeviceConfig {
            device_id: dev_id.to_string(),
            mb_rgb_sync: false,
            zones: vec![],
        });
    }
    rgb.devices.iter_mut().find(|d| d.device_id == dev_id).unwrap()
}

fn get_or_create_zone_config(dev: &mut RgbDeviceConfig, zone: u8) -> &mut RgbZoneConfig {
    if !dev.zones.iter().any(|z| z.zone_index == zone) {
        dev.zones.push(RgbZoneConfig {
            zone_index: zone,
            effect: RgbEffect {
                mode: RgbMode::Static,
                colors: vec![[255, 255, 255]],
                speed: 2,
                brightness: 4,
                direction: RgbDirection::Clockwise,
                scope: RgbScope::All,
            },
            swap_lr: false,
            swap_tb: false,
        });
    }
    dev.zones.iter_mut().find(|z| z.zone_index == zone).unwrap()
}

fn parse_rgb_mode(s: &str) -> RgbMode {
    match s {
        "Off" => RgbMode::Off,
        "Direct" => RgbMode::Direct,
        "Static" => RgbMode::Static,
        "Rainbow" => RgbMode::Rainbow,
        "RainbowMorph" => RgbMode::RainbowMorph,
        "Breathing" => RgbMode::Breathing,
        "Runway" => RgbMode::Runway,
        "Meteor" => RgbMode::Meteor,
        "ColorCycle" => RgbMode::ColorCycle,
        "Staggered" => RgbMode::Staggered,
        "Tide" => RgbMode::Tide,
        "Mixing" => RgbMode::Mixing,
        "Voice" => RgbMode::Voice,
        "Door" => RgbMode::Door,
        "Render" => RgbMode::Render,
        "Ripple" => RgbMode::Ripple,
        "Reflect" => RgbMode::Reflect,
        "TailChasing" => RgbMode::TailChasing,
        "Paint" => RgbMode::Paint,
        "PingPong" => RgbMode::PingPong,
        "Stack" => RgbMode::Stack,
        "CoverCycle" => RgbMode::CoverCycle,
        "Wave" => RgbMode::Wave,
        "Racing" => RgbMode::Racing,
        "Lottery" => RgbMode::Lottery,
        "Intertwine" => RgbMode::Intertwine,
        "MeteorShower" => RgbMode::MeteorShower,
        "Collide" => RgbMode::Collide,
        "ElectricCurrent" => RgbMode::ElectricCurrent,
        "Kaleidoscope" => RgbMode::Kaleidoscope,
        "BigBang" => RgbMode::BigBang,
        "Vortex" => RgbMode::Vortex,
        "Pump" => RgbMode::Pump,
        "ColorsMorph" => RgbMode::ColorsMorph,
        _ => RgbMode::Off,
    }
}

fn parse_rgb_direction(s: &str) -> RgbDirection {
    match s {
        "Clockwise" => RgbDirection::Clockwise,
        "CounterClockwise" => RgbDirection::CounterClockwise,
        "Up" => RgbDirection::Up,
        "Down" => RgbDirection::Down,
        "Spread" => RgbDirection::Spread,
        "Gather" => RgbDirection::Gather,
        _ => RgbDirection::Clockwise,
    }
}

fn parse_rgb_scope(s: &str) -> RgbScope {
    match s {
        "All" => RgbScope::All,
        "Top" => RgbScope::Top,
        "Bottom" => RgbScope::Bottom,
        "Inner" => RgbScope::Inner,
        "Outer" => RgbScope::Outer,
        _ => RgbScope::All,
    }
}
