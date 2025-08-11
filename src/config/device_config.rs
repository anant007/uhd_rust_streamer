//! Device configuration types

use serde::{Deserialize, Serialize};

/// Device configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceConfig {
    pub args: String,
    pub timeout: f64,
    pub clock_source: String,
    pub time_source: String,
    pub pps_reset: PpsResetConfig,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            args: "addr=192.168.10.2".to_string(),
            timeout: 3.0,
            clock_source: "internal".to_string(),
            time_source: "internal".to_string(),
            pps_reset: PpsResetConfig::default(),
        }
    }
}

/// PPS reset configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PpsResetConfig {
    pub enable: bool,
    pub wait_time_sec: f64,
    pub verify_reset: bool,
    pub max_time_after_reset: f64,
}

impl Default for PpsResetConfig {
    fn default() -> Self {
        Self {
            enable: false,
            wait_time_sec: 1.5,
            verify_reset: true,
            max_time_after_reset: 1.0,
        }
    }
}