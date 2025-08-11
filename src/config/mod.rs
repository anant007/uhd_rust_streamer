//! Configuration management module

pub mod device_config;
pub mod graph_config;
pub mod stream_config;
pub mod validation;

use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};

pub use device_config::{DeviceConfig, PpsResetConfig};
pub use graph_config::{GraphConfig, ConnectionConfig, AutoConnectConfig};
pub use stream_config::{StreamConfig, MultiStreamConfig, BufferConfig};

use crate::{Error, Result};

/// System-wide configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemConfig {
    pub device: DeviceConfig,
    pub stream: StreamConfig,
    pub graph: GraphConfig,
    pub logging: LoggingConfig,
    pub performance: PerformanceConfig,
}

impl SystemConfig {
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        serde_yaml::from_str(&content)
            .map_err(|e| Error::ConfigError(format!("Failed to parse config: {}", e)))
    }
    
    /// Save configuration to file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| Error::ConfigError(format!("Failed to serialize config: {}", e)))?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            device: DeviceConfig::default(),
            stream: StreamConfig::default(),
            graph: GraphConfig::default(),
            logging: LoggingConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            format: "pretty".to_string(),
        }
    }
}

/// Performance configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    pub worker_threads: usize,
    pub cpu_affinity: Vec<usize>,
    pub realtime_priority: Option<i32>,
    pub lock_memory: bool,
    pub use_huge_pages: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            worker_threads: 4,
            cpu_affinity: vec![],
            realtime_priority: None,
            lock_memory: false,
            use_huge_pages: false,
        }
    }
}

/// Configuration update message
#[derive(Debug, Clone)]
pub enum ConfigUpdate {
    Device(DeviceConfig),
    Stream(StreamConfig),
    Graph(GraphConfig),
    Logging(LoggingConfig),
    Performance(PerformanceConfig),
}