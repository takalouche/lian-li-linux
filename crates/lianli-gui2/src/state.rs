//! Shared application state, updated by the backend thread and read by UI callbacks.

use lianli_shared::config::AppConfig;
use lianli_shared::ipc::DeviceInfo;
use lianli_shared::rgb::RgbDeviceCapabilities;

#[derive(Debug, Default)]
pub struct SharedState {
    pub config: Option<AppConfig>,
    pub rgb_caps: Vec<RgbDeviceCapabilities>,
    pub devices: Vec<DeviceInfo>,
}
