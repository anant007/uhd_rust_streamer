//! CHDR packet handling

use zerocopy::{AsBytes, FromBytes};

/// CHDR packet types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Management = 0x0,
    StreamStatus = 0x1,
    StreamCommand = 0x2,
    Control = 0x4,
    DataNoTs = 0x6,
    DataWithTs = 0x7,
}

/// CHDR header structure
#[repr(C)]
#[derive(Debug, Clone, Copy, AsBytes, FromBytes)]
pub struct ChdrHeader {
    pub header: u64,
}

impl ChdrHeader {
    /// Parse packet type
    pub fn packet_type(&self) -> PacketType {
        let pkt_type = ((self.header >> 53) & 0x7) as u8;
        match pkt_type {
            0x0 => PacketType::Management,
            0x1 => PacketType::StreamStatus,
            0x2 => PacketType::StreamCommand,
            0x4 => PacketType::Control,
            0x6 => PacketType::DataNoTs,
            0x7 => PacketType::DataWithTs,
            _ => PacketType::DataNoTs,
        }
    }
    
    /// Check if packet has timestamp
    pub fn has_timestamp(&self) -> bool {
        self.packet_type() == PacketType::DataWithTs
    }
    
    /// Get packet length
    pub fn length(&self) -> u16 {
        ((self.header >> 16) & 0xFFFF) as u16
    }
    
    /// Get sequence number
    pub fn seq_num(&self) -> u16 {
        ((self.header >> 32) & 0xFFFF) as u16
    }
}

/// CHDR packet
#[derive(Debug, Clone)]
pub struct ChdrPacket {
    pub header: ChdrHeader,
    pub timestamp: Option<u64>,
    pub payload: Vec<u8>,
}