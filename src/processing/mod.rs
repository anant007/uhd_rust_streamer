//! Signal processing core for RFNoC graph management

pub mod graph;
pub mod streams;
pub mod timing;

use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;
use dashmap::DashMap;
use tracing::{debug, info, warn};

use crate::{SystemMessage, MessageBus, Result, Error};
use crate::config::{SystemConfig, GraphConfig, ConfigUpdate};
use crate::hardware::{RfnocGraph, BlockInfo, GraphEdge, StreamEndpoint};

pub use graph::{GraphManager, GraphTopology};
pub use streams::{StreamCoordinator, StreamHandle};
pub use timing::{TimingManager, PpsSync};

/// Stream data message types
#[derive(Debug, Clone)]
pub enum StreamDataMessage {
    /// Stream has started
    StreamStarted {
        stream_id: usize,
        block_id: String,
        port: usize,
    },
    /// Stream has stopped
    StreamStopped {
        stream_id: usize,
    },
    /// Data is available
    DataAvailable {
        stream_id: usize,
        num_samples: usize,
    },
}

/// Graph processor for managing RFNoC graphs
pub struct GraphProcessor {
    /// System configuration
    config: Arc<RwLock<SystemConfig>>,
    /// Message bus
    message_bus: Arc<MessageBus>,
    /// Graph manager
    graph_manager: Arc<GraphManager>,
    /// Stream coordinator
    stream_coordinator: Arc<StreamCoordinator>,
    /// Timing manager
    timing_manager: Arc<TimingManager>,
    /// Active graphs by device ID
    active_graphs: DashMap<String, Arc<RfnocGraph>>,
}

impl GraphProcessor {
    /// Create a new graph processor
    pub fn new(
        config: Arc<RwLock<SystemConfig>>,
        message_bus: Arc<MessageBus>,
    ) -> Result<Self> {
        let graph_manager = Arc::new(GraphManager::new(config.clone()));
        let stream_coordinator = Arc::new(StreamCoordinator::new(config.clone(), message_bus.clone()));
        let timing_manager = Arc::new(TimingManager::new(config.clone()));
        
        Ok(Self {
            config,
            message_bus,
            graph_manager,
            stream_coordinator,
            timing_manager,
            active_graphs: DashMap::new(),
        })
    }
    
    /// Start the graph processor
    pub async fn start(&self) -> Result<()> {
        info!("Starting graph processor");
        
        // Start timing manager
        self.timing_manager.start().await?;
        
        // Start stream coordinator
        self.stream_coordinator.start().await?;
        
        Ok(())
    }
    
    /// Stop the graph processor
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping graph processor");
        
        // Stop all streams
        self.stream_coordinator.stop_all_streams().await?;
        
        // Stop timing manager
        self.timing_manager.stop().await?;
        
        Ok(())
    }
    
    /// Handle configuration update
    pub async fn handle_config_update(&self, update: ConfigUpdate) -> Result<()> {
        match update {
            ConfigUpdate::Graph(graph_config) => {
                self.apply_graph_config(graph_config).await?;
            }
            _ => {
                // Other updates handled elsewhere
            }
        }
        Ok(())
    }
    
    /// Initialize a graph for a device
    pub async fn initialize_graph(&self, device_id: String) -> Result<Arc<RfnocGraph>> {
        info!("Initializing graph for device: {}", device_id);
        
        // Create RFNoC graph
        let device_args = self.config.read().device.args.clone();
        let mut graph = RfnocGraph::new(&device_args)?;
        
        // Discover topology
        let topology = self.graph_manager.discover_topology(&graph)?;
        info!("Discovered {} blocks", topology.blocks.len());
        
        // Apply graph configuration
        let graph_config = self.config.read().graph.clone();
        self.graph_manager.apply_configuration(&mut graph, &graph_config)?;
        
        // Apply PPS reset if configured
        if self.config.read().device.pps_reset.enable {
            self.timing_manager.perform_pps_reset(&mut graph).await?;
        }
        
        // Commit the graph
        graph.commit()?;
        
        let graph = Arc::new(graph);
        self.active_graphs.insert(device_id.clone(), graph.clone());
        
        Ok(graph)
    }
    
    /// Apply graph configuration
    async fn apply_graph_config(&self, config: GraphConfig) -> Result<()> {
        for mut graph_ref in self.active_graphs.iter_mut() {
            let device_id = graph_ref.key().clone();
            
            // Apply configuration to graph
            // Note: This is simplified - in reality you'd need to handle
            // the Arc<RfnocGraph> -> &mut RfnocGraph conversion properly
            info!("Applying graph configuration to device: {}", device_id);
        }
        
        Ok(())
    }
    
    /// Start streaming from specified endpoints
    pub async fn start_streaming(
        &self,
        device_id: &str,
        endpoints: Vec<StreamEndpoint>,
    ) -> Result<Vec<StreamHandle>> {
        let graph = self.active_graphs.get(device_id)
            .ok_or_else(|| Error::ProcessingError(format!("Device {} not found", device_id)))?
            .clone();
        
        let handles = self.stream_coordinator.start_streams(graph, endpoints).await?;
        
        Ok(handles)
    }
    
    /// Stop streaming
    pub async fn stop_streaming(&self, handles: Vec<StreamHandle>) -> Result<()> {
        self.stream_coordinator.stop_streams(handles).await
    }
    
    /// Get graph topology for a device
    pub fn get_topology(&self, device_id: &str) -> Result<GraphTopology> {
        let graph = self.active_graphs.get(device_id)
            .ok_or_else(|| Error::ProcessingError(format!("Device {} not found", device_id)))?;
        
        self.graph_manager.discover_topology(&graph)
    }
    
    /// Get available stream endpoints
    pub fn get_stream_endpoints(&self, device_id: &str) -> Result<Vec<StreamEndpoint>> {
        let graph = self.active_graphs.get(device_id)
            .ok_or_else(|| Error::ProcessingError(format!("Device {} not found", device_id)))?;
        
        self.graph_manager.find_stream_endpoints(&graph)
    }
    
    /// Modify graph connection
    pub async fn modify_connection(
        &self,
        device_id: &str,
        src_block: &str,
        src_port: usize,
        dst_block: &str,
        dst_port: usize,
    ) -> Result<()> {
        // Get mutable access to graph
        // This is simplified - proper implementation would need careful handling
        info!("Modifying connection: {}:{} -> {}:{}", 
            src_block, src_port, dst_block, dst_port);
        
        Ok(())
    }
    
    /// Get block properties
    pub fn get_block_properties(
        &self,
        device_id: &str,
        block_id: &str,
    ) -> Result<HashMap<String, String>> {
        let graph = self.active_graphs.get(device_id)
            .ok_or_else(|| Error::ProcessingError(format!("Device {} not found", device_id)))?;
        
        self.graph_manager.get_block_properties(&graph, block_id)
    }
    
    /// Set block property
    pub async fn set_block_property(
        &self,
        device_id: &str,
        block_id: &str,
        property: &str,
        value: &str,
    ) -> Result<()> {
        info!("Setting property {} = {} on block {}", property, value, block_id);
        
        // Implementation would set the property on the actual block
        Ok(())
    }
}

/// Graph statistics
#[derive(Debug, Clone)]
pub struct GraphStatistics {
    pub device_id: String,
    pub num_blocks: usize,
    pub num_connections: usize,
    pub active_streams: usize,
    pub tick_rate: f64,
    pub pps_synchronized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_graph_processor_creation() {
        let config = Arc::new(RwLock::new(SystemConfig::default()));
        let (tx, rx) = async_channel::bounded(100);
        let message_bus = Arc::new(MessageBus {
            system_tx: tx,
            system_rx: rx,
            device_channels: DashMap::new(),
        });
        
        let processor = GraphProcessor::new(config, message_bus);
        assert!(processor.is_ok());
    }
}