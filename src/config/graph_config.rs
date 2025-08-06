//! Graph configuration types and utilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Graph configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphConfig {
    /// Dynamic connections
    pub connections: Vec<ConnectionConfig>,
    /// Block properties
    pub block_properties: HashMap<String, HashMap<String, String>>,
    /// Auto-connect settings
    pub auto_connect: AutoConnectConfig,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            connections: Vec::new(),
            block_properties: HashMap::new(),
            auto_connect: AutoConnectConfig::default(),
        }
    }
}

/// Connection configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionConfig {
    pub src_block: String,
    pub src_port: usize,
    pub dst_block: String,
    pub dst_port: usize,
    #[serde(default)]
    pub is_back_edge: bool,
}

/// Auto-connect configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutoConnectConfig {
    #[serde(default)]
    pub radio_to_ddc: bool,
    #[serde(default)]
    pub find_stream_endpoint: bool,
}

impl Default for AutoConnectConfig {
    fn default() -> Self {
        Self {
            radio_to_ddc: false,
            find_stream_endpoint: true,
        }
    }
}