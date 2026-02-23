use lianli_shared::config::AppConfig;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::error;

/// Watches a config file for changes via mtime polling.
/// (Phase 1: simple polling. Phase 6: upgrade to `notify` crate for inotify.)
pub struct ConfigWatcher {
    path: PathBuf,
    last_mtime: Option<SystemTime>,
}

impl ConfigWatcher {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_mtime: None,
        }
    }

    /// Check if the config file has changed. Returns `Some(config)` if it has.
    pub fn check(&mut self, force: bool) -> Option<AppConfig> {
        let metadata = match std::fs::metadata(&self.path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return None; // Config doesn't exist yet — that's fine
            }
            Err(err) => {
                error!("unable to access config {}: {err}", self.path.display());
                return None;
            }
        };

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if !force && self.last_mtime == Some(modified) {
            return None;
        }

        match AppConfig::load(&self.path) {
            Ok(cfg) => {
                self.last_mtime = Some(modified);
                Some(cfg)
            }
            Err(err) => {
                error!("failed to load {}: {err}", self.path.display());
                None
            }
        }
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
