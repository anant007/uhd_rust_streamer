//! Export functionality for different data formats

use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write, BufWriter};
use parking_lot::RwLock;
use crossbeam::channel::{Sender, Receiver, bounded};
use csv::Writer as CsvWriter;
use hdf5::{File as Hdf5File, Group, Dataset};
use tracing::{info, debug, warn, error};

use crate::{Result, Error};
use crate::config::SystemConfig;
use crate::data_manager::storage::StorageBackend;
use crate::data_manager::capture::{PacketBuffer, FileHeader};

/// Export format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// CSV format
    Csv,
    /// MATLAB .mat format (HDF5-based)
    Matlab,
    /// HDF5 format
    Hdf5,
    /// Raw binary
    Raw,
    /// JSON format
    Json,
}

impl std::str::FromStr for ExportFormat {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "csv" => Ok(ExportFormat::Csv),
            "matlab" | "mat" => Ok(ExportFormat::Matlab),
            "hdf5" | "h5" => Ok(ExportFormat::Hdf5),
            "raw" | "bin" => Ok(ExportFormat::Raw),
            "json" => Ok(ExportFormat::Json),
            _ => Err(Error::ValidationError(format!("Unknown export format: {}", s))),
        }
    }
}

/// Export request
#[derive(Debug, Clone)]
pub struct ExportRequest {
    /// Streams to export
    pub streams: Vec<usize>,
    /// Export format
    pub format: ExportFormat,
    /// Output path
    pub output_path: PathBuf,
    /// Time range (start, end) in seconds
    pub time_range: Option<(f64, f64)>,
}

/// Export manager for handling export operations
pub struct ExportManager {
    /// Configuration
    config: Arc<RwLock<SystemConfig>>,
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    /// Export queue sender
    queue_tx: Sender<ExportRequest>,
    /// Export queue receiver
    queue_rx: Receiver<ExportRequest>,
    /// Worker task
    worker_task: Option<smol::Task<Result<()>>>,
}

impl ExportManager {
    /// Create a new export manager
    pub fn new(
        config: Arc<RwLock<SystemConfig>>,
        storage: Arc<dyn StorageBackend>,
    ) -> Self {
        let (queue_tx, queue_rx) = bounded(100);
        
        Self {
            config,
            storage,
            queue_tx,
            queue_rx,
            worker_task: None,
        }
    }
    
    /// Start the export manager
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting export manager");
        
        let queue_rx = self.queue_rx.clone();
        let storage = self.storage.clone();
        let config = self.config.clone();
        
        self.worker_task = Some(smol::spawn(async move {
            export_worker(queue_rx, storage, config).await
        }));
        
        Ok(())
    }
    
    /// Stop the export manager
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping export manager");
        
        if let Some(task) = self.worker_task.take() {
            drop(self.queue_tx.clone()); // Close the channel
            task.await?;
        }
        
        Ok(())
    }
    
    /// Queue an export request
    pub async fn queue_export(&self, request: ExportRequest) -> Result<()> {
        self.queue_tx.send(request)
            .map_err(|_| Error::ChannelError("Export queue full".to_string()))?;
        Ok(())
    }
}

/// Export worker task
async fn export_worker(
    queue_rx: Receiver<ExportRequest>,
    storage: Arc<dyn StorageBackend>,
    config: Arc<RwLock<SystemConfig>>,
) -> Result<()> {
    while let Ok(request) = queue_rx.recv() {
        info!("Processing export request: {:?}", request);
        
        match request.format {
            ExportFormat::Csv => export_csv(&request, storage.as_ref()).await?,
            ExportFormat::Matlab => export_matlab(&request, storage.as_ref()).await?,
            ExportFormat::Hdf5 => export_hdf5(&request, storage.as_ref()).await?,
            ExportFormat::Raw => export_raw(&request, storage.as_ref()).await?,
            ExportFormat::Json => export_json(&request, storage.as_ref()).await?,
        }
        
        info!("Export completed: {:?}", request.output_path);
    }
    
    Ok(())
}

/// Export to CSV format
async fn export_csv(request: &ExportRequest, storage: &dyn StorageBackend) -> Result<()> {
    let file = File::create(&request.output_path)?;
    let mut writer = CsvWriter::from_writer(BufWriter::new(file));
    
    // Write header
    writer.write_record(&[
        "stream_id",
        "packet_num",
        "timestamp_ticks",
        "timestamp_sec",
        "packet_size",
        "first_sample_i",
        "first_sample_q",
    ])?;
    
    // Process each stream
    for stream_id in &request.streams {
        let filename = format!("stream_{}.dat", stream_id);
        
        // Read packets from storage
        let mut reader = storage.open_reader(&filename).await?;
        let header = reader.read_header().await?;
        let tick_rate = header.tick_rate;
        
        let mut packet_num = 0;
        while let Ok(packet) = reader.read_packet().await {
            // Apply time filter if specified
            if let Some((start_time, end_time)) = request.time_range {
                if let Some(timestamp) = packet.timestamp {
                    let time_sec = timestamp as f64 / tick_rate;
                    if time_sec < start_time || time_sec > end_time {
                        continue;
                    }
                }
            }
            
            // Extract first sample (assuming sc16 format)
            let (first_i, first_q) = if packet.data.len() >= 4 {
                let i = i16::from_le_bytes([packet.data[0], packet.data[1]]);
                let q = i16::from_le_bytes([packet.data[2], packet.data[3]]);
                (i, q)
            } else {
                (0, 0)
            };
            
            writer.write_record(&[
                stream_id.to_string(),
                packet_num.to_string(),
                packet.timestamp.unwrap_or(0).to_string(),
                packet.timestamp.map(|t| (t as f64 / tick_rate).to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                packet.data.len().to_string(),
                first_i.to_string(),
                first_q.to_string(),
            ])?;
            
            packet_num += 1;
        }
    }
    
    writer.flush()?;
    Ok(())
}

/// Export to MATLAB format
async fn export_matlab(request: &ExportRequest, storage: &dyn StorageBackend) -> Result<()> {
    let file = Hdf5File::create(&request.output_path)?;
    
    // MATLAB uses HDF5 format with specific conventions
    for stream_id in &request.streams {
        let filename = format!("stream_{}.dat", stream_id);
        let mut reader = storage.open_reader(&filename).await?;
        let header = reader.read_header().await?;
        
        // Create group for this stream
        let group_name = format!("stream_{}", stream_id);
        let group = file.create_group(&group_name)?;
        
        // Write metadata
        group.new_attr::<f64>()
            .create("tick_rate")?
            .write_scalar(&header.tick_rate)?;
        
        group.new_attr::<u8>()
            .create("pps_reset_used")?
            .write_scalar(&(header.pps_reset_used as u8))?;
        
        // Collect data
        let mut timestamps = Vec::new();
        let mut samples_i = Vec::new();
        let mut samples_q = Vec::new();
        
        while let Ok(packet) = reader.read_packet().await {
            // Apply time filter
            if let Some((start_time, end_time)) = request.time_range {
                if let Some(timestamp) = packet.timestamp {
                    let time_sec = timestamp as f64 / header.tick_rate;
                    if time_sec < start_time || time_sec > end_time {
                        continue;
                    }
                }
            }
            
            // Extract samples (assuming sc16)
            for chunk in packet.data.chunks_exact(4) {
                let i = i16::from_le_bytes([chunk[0], chunk[1]]);
                let q = i16::from_le_bytes([chunk[2], chunk[3]]);
                samples_i.push(i as f64);
                samples_q.push(q as f64);
                
                if let Some(ts) = packet.timestamp {
                    timestamps.push(ts as f64 / header.tick_rate);
                }
            }
        }
        
        // Write datasets
        if !samples_i.is_empty() {
            group.new_dataset::<f64>()
                .shape([samples_i.len()])
                .create("samples_i")?
                .write(&samples_i)?;
            
            group.new_dataset::<f64>()
                .shape([samples_q.len()])
                .create("samples_q")?
                .write(&samples_q)?;
            
            if !timestamps.is_empty() {
                group.new_dataset::<f64>()
                    .shape([timestamps.len()])
                    .create("timestamps")?
                    .write(&timestamps)?;
            }
        }
    }
    
    Ok(())
}

/// Export to HDF5 format
async fn export_hdf5(request: &ExportRequest, storage: &dyn StorageBackend) -> Result<()> {
    let file = Hdf5File::create(&request.output_path)?;
    
    // Create root attributes
    file.new_attr::<&str>()
        .create("format")?
        .write_scalar(&"rfnoc_capture")?;
    
    file.new_attr::<u32>()
        .create("version")?
        .write_scalar(&1)?;
    
    // Process each stream
    for stream_id in &request.streams {
        let filename = format!("stream_{}.dat", stream_id);
        let mut reader = storage.open_reader(&filename).await?;
        let header = reader.read_header().await?;
        
        // Create stream group
        let stream_group = file.create_group(&format!("stream_{}", stream_id))?;
        
        // Write stream metadata
        let metadata_group = stream_group.create_group("metadata")?;
        metadata_group.new_attr::<f64>()
            .create("tick_rate")?
            .write_scalar(&header.tick_rate)?;
        
        // Create packet group
        let packets_group = stream_group.create_group("packets")?;
        
        let mut packet_idx = 0;
        while let Ok(packet) = reader.read_packet().await {
            // Apply time filter
            if let Some((start_time, end_time)) = request.time_range {
                if let Some(timestamp) = packet.timestamp {
                    let time_sec = timestamp as f64 / header.tick_rate;
                    if time_sec < start_time || time_sec > end_time {
                        continue;
                    }
                }
            }
            
            // Create packet dataset
            let packet_name = format!("packet_{:06}", packet_idx);
            let packet_dataset = packets_group.new_dataset::<u8>()
                .shape([packet.data.len()])
                .create(&packet_name)?;
            
            packet_dataset.write(&packet.data)?;
            
            // Write packet attributes
            if let Some(ts) = packet.timestamp {
                packet_dataset.new_attr::<u64>()
                    .create("timestamp")?
                    .write_scalar(&ts)?;
            }
            
            packet_idx += 1;
        }
        
        // Write summary info
        stream_group.new_attr::<usize>()
            .create("num_packets")?
            .write_scalar(&packet_idx)?;
    }
    
    Ok(())
}

/// Export to raw binary format
async fn export_raw(request: &ExportRequest, storage: &dyn StorageBackend) -> Result<()> {
    let file = File::create(&request.output_path)?;
    let mut writer = BufWriter::new(file);
    
    // Simple format: just concatenate all sample data
    for stream_id in &request.streams {
        let filename = format!("stream_{}.dat", stream_id);
        let mut reader = storage.open_reader(&filename).await?;
        let header = reader.read_header().await?;
        
        while let Ok(packet) = reader.read_packet().await {
            // Apply time filter
            if let Some((start_time, end_time)) = request.time_range {
                if let Some(timestamp) = packet.timestamp {
                    let time_sec = timestamp as f64 / header.tick_rate;
                    if time_sec < start_time || time_sec > end_time {
                        continue;
                    }
                }
            }
            
            // Write raw sample data (skip CHDR header)
            let sample_start = if packet.timestamp.is_some() { 16 } else { 8 };
            if packet.data.len() > sample_start {
                writer.write_all(&packet.data[sample_start..])?;
            }
        }
    }
    
    writer.flush()?;
    Ok(())
}

/// Export to JSON format
async fn export_json(request: &ExportRequest, storage: &dyn StorageBackend) -> Result<()> {
    use serde_json::{json, Value};
    
    let mut root = json!({
        "format": "rfnoc_capture",
        "version": 1,
        "streams": []
    });
    
    let streams = root["streams"].as_array_mut().unwrap();
    
    for stream_id in &request.streams {
        let filename = format!("stream_{}.dat", stream_id);
        let mut reader = storage.open_reader(&filename).await?;
        let header = reader.read_header().await?;
        
        let mut stream_obj = json!({
            "id": stream_id,
            "tick_rate": header.tick_rate,
            "pps_reset_used": header.pps_reset_used,
            "packets": []
        });
        
        let packets = stream_obj["packets"].as_array_mut().unwrap();
        
        while let Ok(packet) = reader.read_packet().await {
            // Apply time filter
            if let Some((start_time, end_time)) = request.time_range {
                if let Some(timestamp) = packet.timestamp {
                    let time_sec = timestamp as f64 / header.tick_rate;
                    if time_sec < start_time || time_sec > end_time {
                        continue;
                    }
                }
            }
            
            // Extract first few samples for preview
            let mut sample_preview = Vec::new();
            for chunk in packet.data.chunks_exact(4).take(4) {
                let i = i16::from_le_bytes([chunk[0], chunk[1]]);
                let q = i16::from_le_bytes([chunk[2], chunk[3]]);
                sample_preview.push(json!({ "i": i, "q": q }));
            }
            
            packets.push(json!({
                "number": packet.packet_number,
                "timestamp": packet.timestamp,
                "timestamp_sec": packet.timestamp.map(|t| t as f64 / header.tick_rate),
                "size": packet.data.len(),
                "sample_preview": sample_preview
            }));
        }
        
        streams.push(stream_obj);
    }
    
    // Write JSON file
    let file = File::create(&request.output_path)?;
    serde_json::to_writer_pretty(file, &root)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_export_format_parsing() {
        assert_eq!("csv".parse::<ExportFormat>().unwrap(), ExportFormat::Csv);
        assert_eq!("matlab".parse::<ExportFormat>().unwrap(), ExportFormat::Matlab);
        assert_eq!("mat".parse::<ExportFormat>().unwrap(), ExportFormat::Matlab);
        assert_eq!("hdf5".parse::<ExportFormat>().unwrap(), ExportFormat::Hdf5);
        assert_eq!("raw".parse::<ExportFormat>().unwrap(), ExportFormat::Raw);
        assert_eq!("json".parse::<ExportFormat>().unwrap(), ExportFormat::Json);
        
        assert!("unknown".parse::<ExportFormat>().is_err());
    }
}