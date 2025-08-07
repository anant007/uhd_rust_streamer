//! Stream configuration types

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stream configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamConfig {
    /// Output directory
    pub output_directory: PathBuf,
    /// Default tick rate
    pub tick_rate: f64,
    /// Multi-stream configuration
    pub multi_stream: MultiStreamConfig,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            output_directory: PathBuf::from("captures"),
            tick_rate: 200e6,
            multi_stream: MultiStreamConfig::default(),
        }
    }
}

/// Multi-stream configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiStreamConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_true")]
    pub sync_streams: bool,
    #[serde(default)]
    pub separate_files: bool,
    #[serde(default = "default_file_prefix")]
    pub file_prefix: String,
    #[serde(default)]
    pub buffer_config: BufferConfig,
}

impl Default for MultiStreamConfig {
    fn default() -> Self {
        Self {
            enable: false,
            sync_streams: true,
            separate_files: false,
            file_prefix: "stream".to_string(),
            buffer_config: BufferConfig::default(),
        }
    }
}

/// Buffer configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BufferConfig {
    #[serde(default = "default_ring_buffer_size")]
    pub ring_buffer_size: usize,
    #[serde(default = "default_batch_write_size")]
    pub batch_write_size: usize,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            ring_buffer_size: 16 * 1024 * 1024, // 16MB
            batch_write_size: 100,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_file_prefix() -> String {
    "stream".to_string()
}

fn default_ring_buffer_size() -> usize {
    16 * 1024 * 1024
}

fn default_batch_write_size() -> usize {
    100
}