//! Data management layer for capture, storage, and export

pub mod capture;
pub mod export;
pub mod storage;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::collections::HashMap;
use parking_lot::RwLock;
use dashmap::DashMap;
use crossbeam::channel;
use tracing::{debug, info, warn, error};

use crate::{SystemMessage, MessageBus, Result, Error};
use crate::config::SystemConfig;
use crate::processing::StreamDataMessage;

pub use capture::{CaptureManager, StreamBuffer, PacketBuffer};
pub use export::{ExportManager, ExportFormat, ExportRequest};
pub use storage::{StorageBackend, FileStorage};

/// Data manager for coordinating capture, storage, and export
pub struct DataManager {
    /// System configuration
    config: Arc<RwLock<SystemConfig>>,
    /// Message bus for communication
    message_bus: Arc<MessageBus>,
    /// Capture manager
    capture_manager: Arc<CaptureManager>,
    /// Export manager
    export_manager: Arc<ExportManager>,
    /// Storage backend
    storage_backend: Arc<dyn StorageBackend>,
    /// Active streams
    active_streams: DashMap<AtomicUsize, StreamInfo>,
    /// Statistics
    stats: Arc<DataManagerStats>,
}

/// Stream information
#[derive(Debug, Clone)]
struct StreamInfo {
    /// Stream ID
    stream_id: usize,
    /// Block ID
    block_id: String,
    /// Port number
    port: usize,
    /// Buffer reference
    buffer: Arc<StreamBuffer>,
    /// Writer task handle
    writer_task: Option<smol::Task<Result<()>>>,
}

/// Data manager statistics
#[derive(Debug, Default)]
pub struct DataManagerStats {
    /// Total packets captured across all streams
    pub total_packets: AtomicU64,
    /// Total bytes written
    pub total_bytes_written: AtomicU64,
    /// Active stream count
    pub active_streams: AtomicUsize,
    /// Buffer overflows
    pub buffer_overflows: AtomicU64,
    /// Export operations completed
    pub exports_completed: AtomicU64,
}

impl DataManager {
    /// Create a new data manager
    pub fn new(
        config: Arc<RwLock<SystemConfig>>,
        message_bus: Arc<MessageBus>,
    ) -> Result<Self> {
        let storage_backend = Arc::new(FileStorage::new(
            config.read().stream.output_directory.clone()
        )?);
        
        let capture_manager = Arc::new(CaptureManager::new(
            config.clone(),
            message_bus.clone(),
        ));
        
        let export_manager = Arc::new(ExportManager::new(
            config.clone(),
            storage_backend.clone(),
        ));
        
        Ok(Self {
            config,
            message_bus,
            capture_manager,
            export_manager,
            storage_backend,
            active_streams: DashMap::new(),
            stats: Arc::new(DataManagerStats::default()),
        })
    }
    
    /// Start the data manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting data manager");
        
        // Start capture manager
        self.capture_manager.start().await?;
        
        // Start export manager
        self.export_manager.start().await?;
        
        Ok(())
    }
    
    /// Stop the data manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping data manager");
        
        // Stop all active streams
        for mut stream in self.active_streams.iter_mut() {
            if let Some(task) = stream.writer_task.take() {
                task.cancel().await;
            }
        }
        
        // Stop capture manager
        self.capture_manager.stop().await?;
        
        // Stop export manager
        self.export_manager.stop().await?;
        
        Ok(())
    }
    
    /// Handle stream data message
    pub async fn handle_stream_data(&self, msg: StreamDataMessage) -> Result<()> {
        match msg {
            StreamDataMessage::StreamStarted { stream_id, block_id, port } => {
                self.handle_stream_started(stream_id, block_id, port).await?;
            }
            StreamDataMessage::StreamStopped { stream_id } => {
                self.handle_stream_stopped(stream_id).await?;
            }
            StreamDataMessage::DataAvailable { stream_id, num_samples } => {
                self.handle_data_available(stream_id, num_samples).await?;
            }
        }
        Ok(())
    }
    
    /// Handle stream started
    async fn handle_stream_started(
        &self,
        stream_id: usize,
        block_id: String,
        port: usize,
    ) -> Result<()> {
        info!("Stream {} started: {}:{}", stream_id, block_id, port);
        
        // Create buffer for stream
        let buffer = self.capture_manager.create_stream_buffer(stream_id)?;
        
        // Start writer task
        let writer_task = self.spawn_writer_task(stream_id, buffer.clone());
        
        let stream_info = StreamInfo {
            stream_id,
            block_id,
            port,
            buffer,
            writer_task: Some(writer_task),
        };
        
        self.active_streams.insert(stream_id, stream_info);
        self.stats.active_streams.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }
    
    /// Handle stream stopped
    async fn handle_stream_stopped(&self, stream_id: usize) -> Result<()> {
        info!("Stream {} stopped", stream_id);
        
        if let Some(mut stream) = self.active_streams.remove(&stream_id) {
            // Cancel writer task
            if let Some(task) = stream.1.writer_task.take() {
                task.cancel().await;
            }
            
            self.stats.active_streams.fetch_sub(1, Ordering::Relaxed);
        }
        
        Ok(())
    }
    
    /// Handle data available
    async fn handle_data_available(
        &self,
        stream_id: usize,
        num_samples: usize,
    ) -> Result<()> {
        debug!("Stream {} has {} samples available", stream_id, num_samples);
        
        // Update statistics
        self.stats.total_packets.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }
    
    /// Spawn writer task for a stream
    fn spawn_writer_task(
        &self,
        stream_id: usize,
        buffer: Arc<StreamBuffer>,
    ) -> smol::Task<Result<()>> {
        let storage = self.storage_backend.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        
        smol::spawn(async move {
            let filename = format!("stream_{}.dat", stream_id);
            let mut writer = storage.create_writer(&filename).await?;
            
            // Write file header
            let header = capture::FileHeader {
                magic: 0x43484452,
                version: 4,
                stream_id: stream_id as u32,
                tick_rate: config.read().stream.tick_rate,
                pps_reset_used: if config.read().device.pps_reset.enable { 1 } else { 0 },
                _padding: [0; 3],
                pps_reset_time_sec: 0.0,
                reserved: [0; 32],
            };
            writer.write_header(&header).await?;
            
            // Main write loop
            loop {
                match buffer.read_packet_timeout(std::time::Duration::from_millis(100)).await {
                    Ok(packet) => {
                        let bytes_written = writer.write_packet(&packet).await?;
                        stats.total_bytes_written.fetch_add(bytes_written as u64, Ordering::Relaxed);
                    }
                    Err(capture::BufferError::Timeout) => {
                        // Check if we should stop
                        if buffer.is_closed() {
                            break;
                        }
                    }
                    Err(capture::BufferError::Overflow) => {
                        stats.buffer_overflows.fetch_add(1, Ordering::Relaxed);
                        warn!("Buffer overflow on stream {}", stream_id);
                    }
                    Err(e) => {
                        error!("Error reading from buffer: {:?}", e);
                        break;
                    }
                }
            }
            
            writer.close().await?;
            info!("Writer task for stream {} completed", stream_id);
            Ok(())
        })
    }
    
    /// Create an export request
    pub async fn request_export(
        &self,
        streams: Vec<usize>,
        format: ExportFormat,
        output_path: std::path::PathBuf,
    ) -> Result<()> {
        let request = ExportRequest {
            streams,
            format,
            output_path,
            time_range: None,
        };
        
        self.export_manager.queue_export(request).await?;
        Ok(())
    }
    
    /// Get active stream count
    pub fn get_active_stream_count(&self) -> usize {
        self.stats.active_streams.load(Ordering::Relaxed)
    }
    
    /// Get total packet count
    pub fn get_total_packet_count(&self) -> u64 {
        self.stats.total_packets.load(Ordering::Relaxed)
    }
    
    /// Get buffer usage percentage
    pub fn get_buffer_usage(&self) -> f32 {
        let mut total_usage = 0.0;
        let mut count = 0;
        
        for stream in self.active_streams.iter() {
            total_usage += stream.buffer.get_usage_percentage();
            count += 1;
        }
        
        if count > 0 {
            total_usage / count as f32
        } else {
            0.0
        }
    }
    
    /// Get detailed statistics
    pub fn get_statistics(&self) -> DataManagerStatistics {
        DataManagerStatistics {
            total_packets: self.stats.total_packets.load(Ordering::Relaxed),
            total_bytes_written: self.stats.total_bytes_written.load(Ordering::Relaxed),
            active_streams: self.stats.active_streams.load(Ordering::Relaxed),
            buffer_overflows: self.stats.buffer_overflows.load(Ordering::Relaxed),
            exports_completed: self.stats.exports_completed.load(Ordering::Relaxed),
            per_stream_stats: self.get_per_stream_stats(),
        }
    }
    
    /// Get per-stream statistics
    fn get_per_stream_stats(&self) -> HashMap<usize, StreamStatistics> {
        let mut stats = HashMap::new();
        
        for stream in self.active_streams.iter() {
            let stream_stats = StreamStatistics {
                stream_id: stream.stream_id,
                block_id: stream.block_id.clone(),
                port: stream.port,
                packets_captured: stream.buffer.get_packet_count(),
                bytes_written: stream.buffer.get_bytes_written(),
                buffer_usage: stream.buffer.get_usage_percentage(),
                overflows: stream.buffer.get_overflow_count(),
            };
            stats.insert(stream.stream_id, stream_stats);
        }
        
        stats
    }
}

/// Data manager statistics
#[derive(Debug, Clone)]
pub struct DataManagerStatistics {
    pub total_packets: u64,
    pub total_bytes_written: u64,
    pub active_streams: usize,
    pub buffer_overflows: u64,
    pub exports_completed: u64,
    pub per_stream_stats: HashMap<usize, StreamStatistics>,
}

/// Per-stream statistics
#[derive(Debug, Clone)]
pub struct StreamStatistics {
    pub stream_id: usize,
    pub block_id: String,
    pub port: usize,
    pub packets_captured: u64,
    pub bytes_written: u64,
    pub buffer_usage: f32,
    pub overflows: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_data_manager_creation() {
        smol::block_on(async {
            let config = Arc::new(RwLock::new(SystemConfig::default()));
            let (tx, rx) = async_channel::bounded(100);
            let message_bus = Arc::new(MessageBus {
                system_tx: tx,
                system_rx: rx,
                device_channels: DashMap::new(),
            });
            
            let data_manager = DataManager::new(config, message_bus);
            assert!(data_manager.is_ok());
        });
    }
}