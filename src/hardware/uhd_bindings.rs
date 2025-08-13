//! UHD FFI bindings using CXX for safe C++ interop

use cxx::UniquePtr;
use std::pin::Pin;
use std::time::Duration;
use crate::error::{Error, Result};
use std::sync::Arc;
use parking_lot::Mutex;

#[cxx::bridge(namespace = "rfnoc_tool")]
mod ffi {
    /// Stream arguments
    struct StreamArgs {
        cpu_format: String,
        otw_format: String,
        channels: Vec<usize>,
        args: String,
    }
    
    /// Device arguments
    struct DeviceArgs {
        args: String,
    }
    
    /// Time spec
    struct TimeSpec {
        secs: u64,
        nsecs: u32,
    }
    
    /// RX metadata
    struct RxMetadata {
        has_time_spec: bool,
        time_spec: TimeSpec,
        more_fragments: bool,
        fragment_offset: usize,
        start_of_burst: bool,
        end_of_burst: bool,
        error_code: u32,
    }
    
    /// Graph edge
    struct GraphEdgeFFI {
        src_block_id: String,
        src_port: usize,
        dst_block_id: String,
        dst_port: usize,
    }
    
    unsafe extern "C++" {
        include!("src/hardware/uhd_wrapper.h");
        
        type RfnocGraphWrapper;
        type RxStreamerWrapper;
        type BlockControlWrapper;
        
        /// Create a new RFNoC graph
        fn create_rfnoc_graph(args: &DeviceArgs) -> Result<UniquePtr<RfnocGraphWrapper>>;
        
        /// Get available blocks
        fn get_block_ids(self: &RfnocGraphWrapper) -> Vec<String>;
        
        /// Get block
        fn get_block(self: &RfnocGraphWrapper, block_id: &str) -> Result<UniquePtr<BlockControlWrapper>>;
        
        /// Connect blocks
        fn connect_blocks(
            self: Pin<&mut RfnocGraphWrapper>,
            src_block: &str,
            src_port: usize,
            dst_block: &str,
            dst_port: usize,
        ) -> Result<()>;
        
        /// Enumerate connections
        fn enumerate_connections(self: &RfnocGraphWrapper) -> Vec<GraphEdgeFFI>;
        
        /// Commit graph
        fn commit_graph(self: Pin<&mut RfnocGraphWrapper>) -> Result<()>;
        
        /// Create RX streamer
        fn create_rx_streamer(
            self: Pin<&mut RfnocGraphWrapper>,
            stream_args: &StreamArgs,
        ) -> Result<UniquePtr<RxStreamerWrapper>>;
        
        /// Connect streamer
        fn connect_rx_streamer(
            self: Pin<&mut RfnocGraphWrapper>,
            streamer: &RxStreamerWrapper,
            block_id: &str,
            port: usize,
        ) -> Result<()>;
        
        /// Receive samples
        fn recv_samples(
            self: Pin<&mut RxStreamerWrapper>,
            buffs: &mut [u8],
            nsamps_per_buff: usize,
            metadata: &mut RxMetadata,
            timeout: f64,
        ) -> Result<usize>;
        
        /// Issue stream command with time
        fn issue_stream_cmd_timed(
            self: Pin<&mut RxStreamerWrapper>,
            stream_mode: u32,
            num_samps: u64,
            time_secs: u64,
            time_nsecs: u32,
            stream_now: bool,
        ) -> Result<()>;
        
        /// Issue stream command without time
        fn issue_stream_cmd(
            self: Pin<&mut RxStreamerWrapper>,
            stream_mode: u32,
            num_samps: u64,
            stream_now: bool,
        ) -> Result<()>;
        
        /// Get tick rate
        fn get_tick_rate(self: &RfnocGraphWrapper) -> f64;
        
        /// Set time next PPS
        fn set_time_next_pps(self: Pin<&mut RfnocGraphWrapper>, time_spec: &TimeSpec) -> Result<()>;
        
        /// Get time now
        fn get_time_now(self: &RfnocGraphWrapper) -> TimeSpec;
        
        /// Block control operations
        fn get_block_type(self: &BlockControlWrapper) -> String;
        fn get_num_input_ports(self: &BlockControlWrapper) -> usize;
        fn get_num_output_ports(self: &BlockControlWrapper) -> usize;
        fn get_property_names(self: &BlockControlWrapper) -> Vec<String>;
        fn set_property_double(
            self: Pin<&mut BlockControlWrapper>,
            name: &str,
            value: f64,
            port: usize,
        ) -> Result<()>;
        fn get_property_double(self: &BlockControlWrapper, name: &str, port: usize) -> Result<f64>;
    }
}

/// Stream mode constants
pub mod stream_mode {
    pub const START_CONTINUOUS: u32 = 0;
    pub const STOP_CONTINUOUS: u32 = 1;
    pub const NUM_SAMPS_AND_DONE: u32 = 2;
    pub const NUM_SAMPS_AND_MORE: u32 = 3;
}

/// RX metadata error codes
pub mod rx_error {
    pub const NONE: u32 = 0;
    pub const TIMEOUT: u32 = 1;
    pub const LATE_COMMAND: u32 = 2;
    pub const BROKEN_CHAIN: u32 = 3;
    pub const OVERFLOW: u32 = 4;
    pub const ALIGNMENT: u32 = 5;
    pub const BAD_PACKET: u32 = 6;
}

/// Device arguments builder
pub struct DeviceArgs {
    args: std::collections::HashMap<String, String>,
}

impl DeviceArgs {
    /// Create new device args
    pub fn new() -> Self {
        Self {
            args: std::collections::HashMap::new(),
        }
    }
    
    /// Set device address
    pub fn addr(mut self, addr: &str) -> Self {
        self.args.insert("addr".to_string(), addr.to_string());
        self
    }
    
    /// Set device type
    pub fn device_type(mut self, device_type: &str) -> Self {
        self.args.insert("type".to_string(), device_type.to_string());
        self
    }
    
    /// Set master clock rate
    pub fn master_clock_rate(mut self, rate: f64) -> Self {
        self.args.insert("master_clock_rate".to_string(), rate.to_string());
        self
    }
    
    /// Build args string
    pub fn build(&self) -> String {
        self.args
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl Default for DeviceArgs {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream arguments builder
pub struct StreamArgs {
    cpu_format: String,
    otw_format: String,
    channels: Vec<usize>,
    args: std::collections::HashMap<String, String>,
}

impl StreamArgs {
    /// Create new stream args
    pub fn new(cpu_format: &str) -> Self {
        Self {
            cpu_format: cpu_format.to_string(),
            otw_format: "sc16".to_string(),
            channels: vec![0],
            args: std::collections::HashMap::new(),
        }
    }
    
    /// Set over-the-wire format
    pub fn otw_format(mut self, format: &str) -> Self {
        self.otw_format = format.to_string();
        self
    }
    
    /// Set channels
    pub fn channels(mut self, channels: Vec<usize>) -> Self {
        self.channels = channels;
        self
    }
    
    /// Set samples per packet
    pub fn spp(mut self, spp: usize) -> Self {
        self.args.insert("spp".to_string(), spp.to_string());
        self
    }
}

/// RFNoC graph wrapper - NO MUTEX!
// pub struct RfnocGraph {
//     inner: UniquePtr<ffi::RfnocGraphWrapper>,
// }

/// RFNoC graph wrapper with proper thread safety
pub struct RfnocGraph {
    // Use parking_lot for better performance and simpler API
    inner: parking_lot::Mutex<Option<UniquePtr<ffi::RfnocGraphWrapper>>>,
}


// RfnocGraph is Send but not Sync - only one thread can own it at a time
unsafe impl Send for RfnocGraph {}
unsafe impl Sync for RfnocGraph {}

impl RfnocGraph {
    /// Create a new RFNoC graph
    pub fn new(device_args: &str) -> Result<Self> {
        let args = ffi::DeviceArgs {
            args: device_args.to_string(),
        };
        
        let inner = ffi::create_rfnoc_graph(&args)
            .map_err(|e| Error::UhdError(e.to_string()))?;
        
        Ok(Self { 
            inner: parking_lot::Mutex::new(Some(inner)),
        })
    }
    
    /// Get available block IDs
    pub fn get_block_ids(&self) -> Vec<String> {
        self.inner
            .lock()
            .as_ref()
            .map(|inner| inner.get_block_ids())
            .unwrap_or_default()
    }
    
    /// Get a specific block
    pub fn get_block(&self, block_id: &str) -> Result<BlockControl> {
        let guard = self.inner.lock();
        let inner = guard.as_ref()
            .ok_or_else(|| Error::UhdError("Graph not initialized".to_string()))?;
        
        let block = inner.get_block(block_id)
            .map_err(|e| Error::UhdError(e.to_string()))?;
        
        Ok(BlockControl { inner: block })
    }
    
    /// Connect two blocks
    pub fn connect(
        &self,  // Note: &self, not &mut self - better for concurrent use
        src_block: &str,
        src_port: usize,
        dst_block: &str,
        dst_port: usize,
    ) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut()
            .ok_or_else(|| Error::UhdError("Graph not initialized".to_string()))?;

        // let x = 
        
        // SAFETY: The mutex guard ensures the pointer remains valid for the duration
        // let pinned = unsafe { Pin::new_unchecked(inner) };
        
        inner.pin_mut().connect_blocks(src_block, src_port, dst_block, dst_port)
            .map_err(|e| Error::UhdError(e.to_string()))
    }
    
    /// Enumerate active connections
    pub fn enumerate_connections(&self) -> Vec<GraphEdge> {
        self.inner
            .lock()
            .as_ref()
            .map(|inner| {
                inner.enumerate_connections()
                    .into_iter()
                    .map(|edge| GraphEdge {
                        src_block_id: edge.src_block_id,
                        src_port: edge.src_port,
                        dst_block_id: edge.dst_block_id,
                        dst_port: edge.dst_port,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Commit the graph
    pub fn commit(&self) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut()
            .ok_or_else(|| Error::UhdError("Graph not initialized".to_string()))?;
        
        inner.pin_mut().commit_graph()
            .map_err(|e| Error::UhdError(e.to_string()))
    }
    
    /// Get tick rate
    pub fn get_tick_rate(&self) -> f64 {
        self.inner
            .lock()
            .as_ref()
            .map(|inner| inner.get_tick_rate())
            .unwrap_or(0.0)
    }
    
    /// Set time at next PPS
    pub fn set_time_next_pps(&self, time: crate::hardware::TimeSpec) -> Result<()> {
        let time_spec = ffi::TimeSpec {
            secs: time.secs,
            nsecs: time.nsecs,
        };
        
        let mut guard = self.inner.lock();
        let inner = guard.as_mut()
            .ok_or_else(|| Error::UhdError("Graph not initialized".to_string()))?;
        
        inner.pin_mut().set_time_next_pps(&time_spec)
            .map_err(|e| Error::UhdError(e.to_string()))
    }
    
    /// Get current time
    pub fn get_time_now(&self) -> crate::hardware::TimeSpec {
        self.inner
            .lock()
            .as_ref()
            .map(|inner| {
                let time = inner.get_time_now();
                crate::hardware::TimeSpec::new(time.secs, time.nsecs)
            })
            .unwrap_or_else(|| crate::hardware::TimeSpec::new(0, 0))
    }
    
    /// Create RX streamer
    pub fn create_rx_streamer(&self, stream_args: &StreamArgs) -> Result<RxStreamer> {
        let args = ffi::StreamArgs {
            cpu_format: stream_args.cpu_format.clone(),
            otw_format: stream_args.otw_format.clone(),
            channels: stream_args.channels.clone(),
            args: stream_args.args.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(","),
        };
        
        let mut guard = self.inner.lock();
        let inner = guard.as_mut()
            .ok_or_else(|| Error::UhdError("Graph not initialized".to_string()))?;
        
        // let pinned = unsafe { Pin::new_unchecked(inner) };
        let streamer = inner.pin_mut().create_rx_streamer(&args)
            .map_err(|e| Error::UhdError(e.to_string()))?;
        
        Ok(RxStreamer { inner: streamer })
    }
    
    /// Connect RX streamer to a block
    pub fn connect_rx_streamer(
        &self,
        streamer: &RxStreamer,
        block_id: &str,
        port: usize,
    ) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut()
            .ok_or_else(|| Error::UhdError("Graph not initialized".to_string()))?;
        
        inner.pin_mut().connect_rx_streamer(&streamer.inner, block_id, port)
            .map_err(|e| Error::UhdError(e.to_string()))
    }
}

/// Graph edge
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub src_block_id: String,
    pub src_port: usize,
    pub dst_block_id: String,
    pub dst_port: usize,
}

/// Block control wrapper
pub struct BlockControl {
    inner: UniquePtr<ffi::BlockControlWrapper>,
}

impl BlockControl {
    /// Get block type
    pub fn get_block_type(&self) -> String {
        self.inner.get_block_type()
    }
    
    /// Get number of input ports
    pub fn get_num_input_ports(&self) -> usize {
        self.inner.get_num_input_ports()
    }
    
    /// Get number of output ports
    pub fn get_num_output_ports(&self) -> usize {
        self.inner.get_num_output_ports()
    }
    
    /// Get property names
    pub fn get_property_names(&self) -> Vec<String> {
        self.inner.get_property_names()
    }
    
    /// Set double property
    pub fn set_property_double(&mut self, name: &str, value: f64, port: usize) -> Result<()> {
        self.inner
            .pin_mut()
            .set_property_double(name, value, port)
            .map_err(|e| Error::UhdError(e.to_string()))
    }
    
    /// Get double property
    pub fn get_property_double(&self, name: &str, port: usize) -> Result<f64> {
        self.inner
            .get_property_double(name, port)
            .map_err(|e| Error::UhdError(e.to_string()))
    }
}

/// RX streamer wrapper
pub struct RxStreamer {
    inner: UniquePtr<ffi::RxStreamerWrapper>,
}

impl RxStreamer {
    /// Receive samples
    pub fn recv(
        &mut self,
        buffers: &mut [&mut [u8]],
        nsamps_per_buff: usize,
        timeout: Duration,
    ) -> Result<(usize, RxMetadata)> {
        let mut metadata = ffi::RxMetadata {
            has_time_spec: false,
            time_spec: ffi::TimeSpec { secs: 0, nsecs: 0 },
            more_fragments: false,
            fragment_offset: 0,
            start_of_burst: false,
            end_of_burst: false,
            error_code: 0,
        };
        
        // For now, only support single channel
        if buffers.len() != 1 {
            return Err(Error::UhdError("Multi-channel not yet supported".to_string()));
        }
        
        let num_rx = self.inner
            .pin_mut()
            .recv_samples(
                buffers[0],
                nsamps_per_buff,
                &mut metadata,
                timeout.as_secs_f64(),
            )
            .map_err(|e| Error::UhdError(e.to_string()))?;
        
        let rx_metadata = RxMetadata {
            has_time_spec: metadata.has_time_spec,
            time_spec: if metadata.has_time_spec {
                Some(crate::hardware::TimeSpec::new(
                    metadata.time_spec.secs,
                    metadata.time_spec.nsecs,
                ))
            } else {
                None
            },
            more_fragments: metadata.more_fragments,
            fragment_offset: metadata.fragment_offset,
            start_of_burst: metadata.start_of_burst,
            end_of_burst: metadata.end_of_burst,
            error_code: metadata.error_code,
        };
        
        Ok((num_rx, rx_metadata))
    }
    
    /// Issue stream command
    pub fn issue_stream_cmd(&mut self, cmd: StreamCommand) -> Result<()> {
        match cmd {
            StreamCommand::StartContinuous => {
                self.inner
                    .pin_mut()
                    .issue_stream_cmd(stream_mode::START_CONTINUOUS, 0, true)
                    .map_err(|e| Error::UhdError(e.to_string()))
            }
            StreamCommand::StartNumSamps(n) => {
                self.inner
                    .pin_mut()
                    .issue_stream_cmd(stream_mode::NUM_SAMPS_AND_DONE, n, true)
                    .map_err(|e| Error::UhdError(e.to_string()))
            }
            StreamCommand::StopContinuous => {
                self.inner
                    .pin_mut()
                    .issue_stream_cmd(stream_mode::STOP_CONTINUOUS, 0, true)
                    .map_err(|e| Error::UhdError(e.to_string()))
            }
            StreamCommand::StartAtTime(n, time) => {
                self.inner
                    .pin_mut()
                    .issue_stream_cmd_timed(
                        stream_mode::NUM_SAMPS_AND_DONE,
                        n,
                        time.secs,
                        time.nsecs,
                        false,
                    )
                    .map_err(|e| Error::UhdError(e.to_string()))
            }
        }
    }
}

/// RX metadata
#[derive(Debug, Clone)]
pub struct RxMetadata {
    pub has_time_spec: bool,
    pub time_spec: Option<crate::hardware::TimeSpec>,
    pub more_fragments: bool,
    pub fragment_offset: usize,
    pub start_of_burst: bool,
    pub end_of_burst: bool,
    pub error_code: u32,
}

impl RxMetadata {
    /// Check if there was an error
    pub fn has_error(&self) -> bool {
        self.error_code != rx_error::NONE
    }
    
    /// Get error description
    pub fn error_string(&self) -> Option<&'static str> {
        match self.error_code {
            rx_error::NONE => None,
            rx_error::TIMEOUT => Some("Timeout"),
            rx_error::LATE_COMMAND => Some("Late command"),
            rx_error::BROKEN_CHAIN => Some("Broken chain"),
            rx_error::OVERFLOW => Some("Overflow"),
            rx_error::ALIGNMENT => Some("Alignment error"),
            rx_error::BAD_PACKET => Some("Bad packet"),
            _ => Some("Unknown error"),
        }
    }
}

/// Stream command
#[derive(Debug, Clone)]
pub enum StreamCommand {
    StartContinuous,
    StartNumSamps(u64),
    StopContinuous,
    StartAtTime(u64, crate::hardware::TimeSpec),
}