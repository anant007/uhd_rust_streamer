//! RFNoC Tool - High-performance RFNoC streaming and analysis tool
//!
//! This library provides a comprehensive toolkit for working with USRP devices
//! and RFNoC graphs, including multi-stream capture, PPS synchronization,
//! and real-time analysis capabilities.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod config;
pub mod data_manager;
pub mod hardware;
pub mod interface;
pub mod logging;
pub mod processing;
pub mod error;

use std::sync::Arc;
use async_channel::{Receiver, Sender};
use dashmap::DashMap;
use parking_lot::RwLock;
use smol::Task;
use tracing::{debug, info, warn};

pub use error::{Error, Result};
use crate::config::SystemConfig;
use crate::data_manager::DataManager;
use crate::hardware::DeviceManager;
use crate::processing::GraphProcessor;

/// System-wide message types for actor communication
#[derive(Debug, Clone)]
pub enum SystemMessage {
    /// Device status update
    DeviceStatus(hardware::DeviceStatus),
    /// Stream data available
    StreamData(processing::StreamDataMessage),
    /// Configuration change
    ConfigUpdate(config::ConfigUpdate),
    /// Error occurred
    Error(String),
    /// Shutdown signal
    Shutdown,
}

/// Core coordination system for the RFNoC tool
pub struct RFNoCSystem {
    /// Device manager for hardware interaction
    pub device_manager: Arc<DeviceManager>,
    /// Graph processor for RFNoC graph management
    pub graph_processor: Arc<GraphProcessor>,
    /// Data manager for capture and export
    pub data_manager: Arc<DataManager>,
    /// System configuration
    pub config: Arc<RwLock<SystemConfig>>,
    /// Message bus for actor communication
    message_bus: Arc<MessageBus>,
    /// Active tasks
    tasks: Arc<DashMap<String, Task<Result<()>>>>,
}

/// Message bus for actor communication
pub struct MessageBus {
    /// System message sender
    pub system_tx: Sender<SystemMessage>,
    /// System message receiver
    pub system_rx: Receiver<SystemMessage>,
    /// Device-specific channels
    device_channels: DashMap<String, (Sender<SystemMessage>, Receiver<SystemMessage>)>,
}

impl RFNoCSystem {
    /// Create a new RFNoC system with the given configuration
    pub async fn new(config: SystemConfig) -> Result<Self> {
        info!("Initializing RFNoC system");
        
        let (system_tx, system_rx) = async_channel::bounded(1024);
        let message_bus = Arc::new(MessageBus {
            system_tx,
            system_rx,
            device_channels: DashMap::new(),
        });
        
        let config = Arc::new(RwLock::new(config));
        
        // Initialize subsystems
        let device_manager = Arc::new(
            DeviceManager::new(config.clone(), message_bus.clone()).await?
        );
        
        let graph_processor = Arc::new(
            GraphProcessor::new(config.clone(), message_bus.clone())?
        );
        
        let data_manager = Arc::new(
            DataManager::new(config.clone(), message_bus.clone())?
        );
        
        let tasks = Arc::new(DashMap::new());
        
        Ok(Self {
            device_manager,
            graph_processor,
            data_manager,
            config,
            message_bus,
            tasks,
        })
    }
    
    /// Start the system and all subsystems
    pub async fn start(&self) -> Result<()> {
        info!("Starting RFNoC system");
        
        // Start message bus handler
        let message_handler = self.spawn_message_handler();
        self.tasks.insert("message_handler".to_string(), message_handler);
        
        // Start subsystems
        self.device_manager.start().await?;
        self.graph_processor.start().await?;
        self.data_manager.start().await?;
        
        info!("RFNoC system started successfully");
        Ok(())
    }
    
    /// Stop the system and all subsystems
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping RFNoC system");
        
        // Send shutdown signal
        self.message_bus.system_tx.send(SystemMessage::Shutdown).await
            .map_err(|e| Error::SystemError(format!("Failed to send shutdown: {}", e)))?;
        
        // Stop subsystems
        self.data_manager.stop().await?;
        self.graph_processor.stop().await?;
        self.device_manager.stop().await?;
        
        // Cancel all tasks
        for task in self.tasks.iter() {
            debug!("Cancelling task: {}", task.key());
            task.value().cancel().await;
        }
        
        info!("RFNoC system stopped");
        Ok(())
    }
    
    /// Spawn the main message handler task
    fn spawn_message_handler(&self) -> Task<Result<()>> {
        let device_manager = self.device_manager.clone();
        let graph_processor = self.graph_processor.clone();
        let data_manager = self.data_manager.clone();
        let system_rx = self.message_bus.system_rx.clone();
        
        smol::spawn(async move {
            loop {
                match system_rx.recv().await {
                    Ok(SystemMessage::Shutdown) => {
                        debug!("Message handler received shutdown signal");
                        break;
                    }
                    Ok(msg) => {
                        if let Err(e) = Self::handle_message(
                            msg,
                            &device_manager,
                            &graph_processor,
                            &data_manager,
                        ).await {
                            warn!("Error handling message: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Message bus error: {}", e);
                        break;
                    }
                }
            }
            Ok(())
        })
    }
    
    /// Handle a system message
    async fn handle_message(
        msg: SystemMessage,
        device_manager: &Arc<DeviceManager>,
        graph_processor: &Arc<GraphProcessor>,
        data_manager: &Arc<DataManager>,
    ) -> Result<()> {
        match msg {
            SystemMessage::DeviceStatus(status) => {
                device_manager.handle_status_update(status).await?;
            }
            SystemMessage::StreamData(data) => {
                data_manager.handle_stream_data(data).await?;
            }
            SystemMessage::ConfigUpdate(update) => {
                graph_processor.handle_config_update(update).await?;
            }
            SystemMessage::Error(error) => {
                warn!("System error: {}", error);
            }
            SystemMessage::Shutdown => {
                // Handled in message loop
            }
        }
        Ok(())
    }
    
    /// Get system statistics
    pub async fn get_stats(&self) -> SystemStats {
        SystemStats {
            device_count: self.device_manager.get_device_count(),
            active_streams: self.data_manager.get_active_stream_count(),
            total_packets: self.data_manager.get_total_packet_count(),
            buffer_usage: self.data_manager.get_buffer_usage(),
        }
    }
}

/// System-wide statistics
#[derive(Debug, Clone)]
pub struct SystemStats {
    /// Number of connected devices
    pub device_count: usize,
    /// Number of active streams
    pub active_streams: usize,
    /// Total packets captured
    pub total_packets: u64,
    /// Buffer usage percentage
    pub buffer_usage: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_system_creation() {
        // Test system creation with default config
        smol::block_on(async {
            let config = SystemConfig::default();
            let system = RFNoCSystem::new(config).await;
            assert!(system.is_ok());
        });
    }
}