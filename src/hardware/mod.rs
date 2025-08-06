//! Hardware abstraction layer for USRP devices

pub mod uhd_bindings;
pub mod device_manager;
pub mod chdr_packets;

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use async_channel::Sender;
use tracing::{debug, info, warn};

pub use device_manager::DeviceManager;
pub use chdr_packets::{ChdrPacket, ChdrHeader, PacketType};
pub use uhd_bindings::{RfnocGraph, StreamArgs, DeviceArgs};

use crate::{SystemMessage, MessageBus, Result, Error};
use crate::config::SystemConfig;

/// Device status information
#[derive(Debug, Clone)]
pub struct DeviceStatus {
    /// Device identifier
    pub device_id: String,
    /// Device type
    pub device_type: String,
    /// Connection status
    pub connected: bool,
    /// Current sample rate
    pub sample_rate: f64,
    /// Center frequency
    pub center_freq: f64,
    /// Gain
    pub gain: f64,
    /// Temperature (if available)
    pub temperature: Option<f64>,
    /// GPS lock status
    pub gps_locked: Option<bool>,
    /// Reference lock status
    pub ref_locked: bool,
    /// Timestamp
    pub timestamp: std::time::Instant,
}

/// Hardware capabilities
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// Maximum sample rate
    pub max_sample_rate: f64,
    /// Minimum sample rate
    pub min_sample_rate: f64,
    /// Frequency range
    pub freq_range: (f64, f64),
    /// Gain range
    pub gain_range: (f64, f64),
    /// Number of channels
    pub num_channels: usize,
    /// Available antennas
    pub antennas: Vec<String>,
    /// Available sensors
    pub sensors: Vec<String>,
}

/// Stream endpoint information
#[derive(Debug, Clone)]
pub struct StreamEndpoint {
    /// Block ID
    pub block_id: String,
    /// Port number
    pub port: usize,
    /// Direction (RX/TX)
    pub direction: StreamDirection,
    /// Data format
    pub format: DataFormat,
    /// Active status
    pub active: bool,
}

/// Stream direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    /// Receive
    Rx,
    /// Transmit
    Tx,
}

/// Data format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    /// Complex 16-bit integers
    Sc16,
    /// Complex 32-bit floats
    Fc32,
    /// Complex 64-bit floats
    Fc64,
}

impl DataFormat {
    /// Get bytes per sample
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            DataFormat::Sc16 => 4,  // 2 bytes real + 2 bytes imag
            DataFormat::Fc32 => 8,  // 4 bytes real + 4 bytes imag
            DataFormat::Fc64 => 16, // 8 bytes real + 8 bytes imag
        }
    }
    
    /// Convert to UHD format string
    pub fn to_uhd_string(&self) -> &'static str {
        match self {
            DataFormat::Sc16 => "sc16",
            DataFormat::Fc32 => "fc32",
            DataFormat::Fc64 => "fc64",
        }
    }
}

/// Block information
#[derive(Debug, Clone)]
pub struct BlockInfo {
    /// Block ID
    pub block_id: String,
    /// Block type
    pub block_type: String,
    /// Number of input ports
    pub num_input_ports: usize,
    /// Number of output ports
    pub num_output_ports: usize,
    /// Has stream endpoint
    pub has_stream_endpoint: bool,
    /// Properties
    pub properties: Vec<String>,
    /// Property types
    pub property_types: std::collections::HashMap<String, String>,
}

/// Graph edge information
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Source block ID
    pub src_block_id: String,
    /// Source port
    pub src_port: usize,
    /// Destination block ID
    pub dst_block_id: String,
    /// Destination port
    pub dst_port: usize,
}

/// Device handle trait
pub trait DeviceHandle: Send + Sync {
    /// Get device ID
    fn get_device_id(&self) -> String;
    
    /// Get device status
    fn get_status(&self) -> DeviceStatus;
    
    /// Get hardware capabilities
    fn get_capabilities(&self) -> HardwareCapabilities;
    
    /// Set sample rate
    fn set_sample_rate(&mut self, rate: f64) -> Result<f64>;
    
    /// Set center frequency
    fn set_center_frequency(&mut self, freq: f64, chan: usize) -> Result<f64>;
    
    /// Set gain
    fn set_gain(&mut self, gain: f64, chan: usize) -> Result<f64>;
    
    /// Get sensor value
    fn get_sensor(&self, name: &str) -> Result<String>;
    
    /// Issue stream command
    fn issue_stream_cmd(&mut self, cmd: StreamCommand) -> Result<()>;
}

/// Stream command
#[derive(Debug, Clone)]
pub enum StreamCommand {
    /// Start continuous streaming
    StartContinuous,
    /// Start streaming with specific number of samples
    StartNumSamps(u64),
    /// Stop streaming
    Stop,
}

/// Time specification
#[derive(Debug, Clone, Copy)]
pub struct TimeSpec {
    /// Seconds
    pub secs: u64,
    /// Fractional seconds (nanoseconds)
    pub nsecs: u32,
}

impl TimeSpec {
    /// Create a new time spec
    pub fn new(secs: u64, nsecs: u32) -> Self {
        Self { secs, nsecs }
    }
    
    /// Create from floating point seconds
    pub fn from_secs_f64(secs: f64) -> Self {
        let whole_secs = secs as u64;
        let frac_secs = secs - whole_secs as f64;
        let nsecs = (frac_secs * 1e9) as u32;
        Self { secs: whole_secs, nsecs }
    }
    
    /// Convert to floating point seconds
    pub fn to_secs_f64(&self) -> f64 {
        self.secs as f64 + (self.nsecs as f64 / 1e9)
    }
    
    /// Get current time
    pub fn now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        Self {
            secs: now.as_secs(),
            nsecs: now.subsec_nanos(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_data_format() {
        assert_eq!(DataFormat::Sc16.bytes_per_sample(), 4);
        assert_eq!(DataFormat::Fc32.bytes_per_sample(), 8);
        assert_eq!(DataFormat::Fc64.bytes_per_sample(), 16);
        
        assert_eq!(DataFormat::Sc16.to_uhd_string(), "sc16");
    }
    
    #[test]
    fn test_time_spec() {
        let ts = TimeSpec::new(100, 500_000_000);
        assert_eq!(ts.to_secs_f64(), 100.5);
        
        let ts2 = TimeSpec::from_secs_f64(123.456);
        assert_eq!(ts2.secs, 123);
        assert!((ts2.nsecs as f64 - 456_000_000.0).abs() < 1000.0);
    }
}