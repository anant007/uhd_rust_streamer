//! Capture management with ring buffers and packet handling

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use rtrb::{Producer, Consumer, RingBuffer};
use zerocopy::{AsBytes, FromBytes, FromZeroes};
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn, error};

use crate::{SystemMessage, MessageBus, Result, Error};
use crate::config::SystemConfig;
use crate::hardware::{ChdrPacket, TimeSpec};

/// Buffer statistics
#[derive(Debug, Default)]
pub struct BufferStats {
    pub packets_written: AtomicU64,
    pub bytes_written: AtomicU64,
    pub packets_read: AtomicU64,
    pub bytes_read: AtomicU64,
    pub overflows: AtomicU64,
    pub max_usage: AtomicU64,
}

/// Stream buffer for ring buffer operations
pub struct StreamBuffer {
    stream_id: usize,
    producer: Producer<PacketBuffer>,
    consumer: Consumer<PacketBuffer>,
    stats: Arc<BufferStats>,
    closed: AtomicBool,
}

/// Packet buffer for ring buffer storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketBuffer {
    /// Stream ID
    pub stream_id: u32,
    /// Packet number
    pub packet_number: u64,
    /// Timestamp (if present)
    pub timestamp: Option<u64>,
    /// Packet data
    pub data: Vec<u8>,
}

/// File header for captured data
#[repr(C)]
#[derive(Debug, Clone, Copy, IntoBytes, FromBytes, FromZeros, Pod, Zeroable)]
pub struct FileHeader {
    /// Magic number (0x43484452 = "CHDR")
    pub magic: u32,
    /// Version number
    pub version: u32,
    /// Stream ID
    pub stream_id: u32,
    /// Tick rate
    pub tick_rate: f64,
    /// PPS reset used (as u8 for POD compatibility)
    pub pps_reset_used: u8,
    /// Padding for alignment
    pub _padding: [u8; 3],
    /// PPS reset time
    pub pps_reset_time_sec: f64,
    /// Reserved for future use
    pub reserved: [u8; 32],
}

impl Default for FileHeader {
    fn default() -> Self {
        Self {
            magic: 0x43484452,
            version: 4,
            stream_id: 0,
            tick_rate: 200e6,
            pps_reset_used: 0,
            _padding: [0; 3],
            pps_reset_time_sec: 0.0,
            reserved: [0; 32],
        }
    }
}

/// Buffer error types
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("Buffer overflow")]
    Overflow,
    #[error("Buffer closed")]
    Closed,
    #[error("Timeout")]
    Timeout,
}

impl StreamBuffer {
    /// Create a new stream buffer
    pub fn new(stream_id: usize, capacity: usize) -> (Arc<Self>, Arc<Self>) {
        let (producer, consumer) = RingBuffer::new(capacity);
        
        let stats = Arc::new(BufferStats::default());
        
        let writer = Arc::new(Self {
            stream_id,
            producer,
            consumer: unsafe { std::mem::zeroed() }, // Will be replaced
            stats: stats.clone(),
            closed: AtomicBool::new(false),
        });
        
        let reader = Arc::new(Self {
            stream_id,
            producer: unsafe { std::mem::zeroed() }, // Will be replaced
            consumer,
            stats,
            closed: AtomicBool::new(false),
        });
        
        (writer, reader)
    }
    
    /// Write a packet to the buffer
    pub fn write_packet(&self, packet: PacketBuffer) -> Result<(), BufferError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(BufferError::Closed);
        }
        
        let packet_size = packet.data.len();
        
        match self.producer.push(packet) {
            Ok(()) => {
                self.stats.packets_written.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_written.fetch_add(packet_size as u64, Ordering::Relaxed);
                
                // Update max usage
                let current_usage = self.producer.slots() as u64;
                let mut max_usage = self.stats.max_usage.load(Ordering::Relaxed);
                while current_usage > max_usage {
                    match self.stats.max_usage.compare_exchange_weak(
                        max_usage,
                        current_usage,
                        Ordering::Release,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(x) => max_usage = x,
                    }
                }
                
                Ok(())
            }
            Err(_) => {
                self.stats.overflows.fetch_add(1, Ordering::Relaxed);
                Err(BufferError::Overflow)
            }
        }
    }
    
    /// Read a packet from the buffer
    pub fn read_packet(&self) -> Result<PacketBuffer, BufferError> {
        match self.consumer.pop() {
            Ok(packet) => {
                let packet_size = packet.data.len();
                self.stats.packets_read.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_read.fetch_add(packet_size as u64, Ordering::Relaxed);
                Ok(packet)
            }
            Err(_) => {
                if self.closed.load(Ordering::Acquire) {
                    Err(BufferError::Closed)
                } else {
                    Err(BufferError::Timeout)
                }
            }
        }
    }
    
    /// Read a packet with timeout
    pub async fn read_packet_timeout(&self, timeout: Duration) -> Result<PacketBuffer, BufferError> {
        let start = Instant::now();
        
        loop {
            match self.read_packet() {
                Ok(packet) => return Ok(packet),
                Err(BufferError::Timeout) => {
                    if start.elapsed() > timeout {
                        return Err(BufferError::Timeout);
                    }
                    // Small async yield
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
    
    /// Close the buffer
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
    
    /// Check if buffer is closed
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
    
    /// Get current usage percentage
    pub fn get_usage_percentage(&self) -> f32 {
        let capacity = self.producer.capacity();
        let used = self.producer.slots();
        (used as f32 / capacity as f32) * 100.0
    }
    
    /// Get packet count
    pub fn get_packet_count(&self) -> u64 {
        self.stats.packets_written.load(Ordering::Relaxed)
    }
    
    /// Get bytes written
    pub fn get_bytes_written(&self) -> u64 {
        self.stats.bytes_written.load(Ordering::Relaxed)
    }
    
    /// Get overflow count
    pub fn get_overflow_count(&self) -> u64 {
        self.stats.overflows.load(Ordering::Relaxed)
    }
}

/// Capture manager for coordinating stream capture
pub struct CaptureManager {
    /// Configuration
    config: Arc<RwLock<SystemConfig>>,
    /// Message bus
    message_bus: Arc<MessageBus>,
    /// Active capture sessions
    sessions: Arc<RwLock<Vec<CaptureSession>>>,
}

/// Individual capture session
struct CaptureSession {
    /// Session ID
    id: u64,
    /// Stream buffers
    buffers: Vec<(Arc<StreamBuffer>, Arc<StreamBuffer>)>,
    /// Capture task handle
    task: Option<smol::Task<Result<()>>>,
    /// Start time
    start_time: Instant,
}

impl CaptureManager {
    /// Create a new capture manager
    pub fn new(
        config: Arc<RwLock<SystemConfig>>,
        message_bus: Arc<MessageBus>,
    ) -> Self {
        Self {
            config,
            message_bus,
            sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Start the capture manager
    pub async fn start(&self) -> Result<()> {
        info!("Capture manager started");
        Ok(())
    }
    
    /// Stop the capture manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping capture manager");
        
        // Stop all active sessions
        let mut sessions = self.sessions.write();
        for session in sessions.iter_mut() {
            if let Some(task) = session.task.take() {
                task.cancel().await;
            }
            
            // Close all buffers
            for (writer, _) in &session.buffers {
                writer.close();
            }
        }
        sessions.clear();
        
        Ok(())
    }
    
    /// Create a stream buffer
    pub fn create_stream_buffer(&self, stream_id: usize) -> Result<Arc<StreamBuffer>> {
        let config = self.config.read();
        let buffer_size = config.stream.multi_stream.buffer_config.ring_buffer_size;
        
        // Calculate buffer capacity in packets
        // Assuming average packet size of 8KB
        let capacity = buffer_size / 8192;
        let capacity = capacity.next_power_of_two(); // rtrb requires power of 2
        
        let (writer, reader) = StreamBuffer::new(stream_id, capacity);
        
        // Store the reader part and return writer
        // In real implementation, you'd manage both parts properly
        Ok(writer)
    }
    
    /// Start a capture session
    pub async fn start_capture(
        &self,
        stream_count: usize,
        duration: Option<Duration>,
        packet_limit: Option<u64>,
    ) -> Result<u64> {
        let session_id = self.generate_session_id();
        
        let mut buffers = Vec::new();
        for i in 0..stream_count {
            let (writer, reader) = StreamBuffer::new(i, 65536); // 64K packets
            buffers.push((writer, reader));
        }
        
        let session = CaptureSession {
            id: session_id,
            buffers,
            task: None,
            start_time: Instant::now(),
        };
        
        self.sessions.write().push(session);
        
        info!("Started capture session {} with {} streams", session_id, stream_count);
        Ok(session_id)
    }
    
    /// Stop a capture session
    pub async fn stop_capture(&self, session_id: u64) -> Result<()> {
        let mut sessions = self.sessions.write();
        
        if let Some(pos) = sessions.iter().position(|s| s.id == session_id) {
            let mut session = sessions.remove(pos);
            
            if let Some(task) = session.task.take() {
                task.cancel().await;
            }
            
            for (writer, _) in &session.buffers {
                writer.close();
            }
            
            let duration = session.start_time.elapsed();
            info!("Stopped capture session {} after {:.2}s", session_id, duration.as_secs_f64());
        }
        
        Ok(())
    }
    
    /// Generate a unique session ID
    fn generate_session_id(&self) -> u64 {
        use std::sync::atomic::AtomicU64;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}

/// Packet processor for handling CHDR packets
pub struct PacketProcessor {
    /// Stream ID
    stream_id: u32,
    /// Tick rate for timestamp conversion
    tick_rate: f64,
    /// Packet counter
    packet_count: u64,
    /// Last timestamp
    last_timestamp: Option<u64>,
}

impl PacketProcessor {
    /// Create a new packet processor
    pub fn new(stream_id: u32, tick_rate: f64) -> Self {
        Self {
            stream_id,
            tick_rate,
            packet_count: 0,
            last_timestamp: None,
        }
    }
    
    /// Process a raw packet
    pub fn process_packet(&mut self, data: &[u8]) -> Result<PacketBuffer> {
        // Parse CHDR header (first 8 bytes)
        if data.len() < 8 {
            return Err(Error::ProcessingError("Packet too small".to_string()));
        }
        
        let header = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]);
        
        // Extract timestamp if present
        let has_timestamp = (header >> 61) & 0x1 == 1;
        let timestamp = if has_timestamp && data.len() >= 16 {
            Some(u64::from_le_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15],
            ]))
        } else {
            None
        };
        
        // Validate timestamp continuity
        if let (Some(ts), Some(last_ts)) = (timestamp, self.last_timestamp) {
            let expected_delta = (self.tick_rate * 0.001) as u64; // 1ms worth of ticks
            let actual_delta = ts.saturating_sub(last_ts);
            
            if actual_delta > expected_delta * 10 {
                warn!("Large timestamp gap detected: {} ticks", actual_delta);
            }
        }
        
        self.last_timestamp = timestamp;
        
        let packet = PacketBuffer {
            stream_id: self.stream_id,
            packet_number: self.packet_count,
            timestamp,
            data: data.to_vec(),
        };
        
        self.packet_count += 1;
        
        Ok(packet)
    }
    
    /// Get packet statistics
    pub fn get_stats(&self) -> PacketStats {
        PacketStats {
            stream_id: self.stream_id,
            packet_count: self.packet_count,
            last_timestamp: self.last_timestamp,
        }
    }
}

/// Packet statistics
#[derive(Debug, Clone)]
pub struct PacketStats {
    pub stream_id: u32,
    pub packet_count: u64,
    pub last_timestamp: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stream_buffer() {
        let (writer, reader) = StreamBuffer::new(0, 16);
        
        // Write a packet
        let packet = PacketBuffer {
            stream_id: 0,
            packet_number: 1,
            timestamp: Some(1000),
            data: vec![0; 100],
        };
        
        assert!(writer.write_packet(packet.clone()).is_ok());
        assert_eq!(writer.get_packet_count(), 1);
        assert_eq!(writer.get_bytes_written(), 100);
        
        // Read the packet
        let read_packet = reader.read_packet().unwrap();
        assert_eq!(read_packet.packet_number, packet.packet_number);
        assert_eq!(read_packet.data.len(), packet.data.len());
    }
    
    #[test]
    fn test_buffer_overflow() {
        let (writer, _reader) = StreamBuffer::new(0, 2); // Very small buffer
        
        // Fill the buffer
        for i in 0..3 {
            let packet = PacketBuffer {
                stream_id: 0,
                packet_number: i,
                timestamp: None,
                data: vec![0; 10],
            };
            
            if i < 2 {
                assert!(writer.write_packet(packet).is_ok());
            } else {
                // Should overflow
                assert!(matches!(writer.write_packet(packet), Err(BufferError::Overflow)));
                assert_eq!(writer.get_overflow_count(), 1);
            }
        }
    }
}