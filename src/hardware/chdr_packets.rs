//! CHDR packet handling implementation

use zerocopy::{AsBytes, FromBytes, FromZeroes};
use bytemuck::{Pod, Zeroable};

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

/// CHDR header structure (64-bit)
#[repr(C)]
#[derive(Debug, Clone, Copy, AsBytes, FromBytes, FromZeroes, Pod, Zeroable)]
pub struct ChdrHeader {
    pub header: u64,
}

impl ChdrHeader {
    /// Create a new CHDR header
    pub fn new() -> Self {
        Self { header: 0 }
    }
    
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
    
    /// Get EPID
    pub fn epid(&self) -> u16 {
        (self.header & 0xFFFF) as u16
    }
    
    /// Get EOB flag
    pub fn eob(&self) -> bool {
        ((self.header >> 57) & 0x1) != 0
    }
    
    /// Set packet type
    pub fn set_packet_type(&mut self, pkt_type: PacketType) {
        self.header &= !(0x7 << 53);
        self.header |= ((pkt_type as u64) & 0x7) << 53;
    }
    
    /// Set length
    pub fn set_length(&mut self, length: u16) {
        self.header &= !(0xFFFF << 16);
        self.header |= ((length as u64) & 0xFFFF) << 16;
    }
    
    /// Set sequence number
    pub fn set_seq_num(&mut self, seq_num: u16) {
        self.header &= !(0xFFFF << 32);
        self.header |= ((seq_num as u64) & 0xFFFF) << 32;
    }
}

impl Default for ChdrHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// CHDR packet
#[derive(Debug, Clone)]
pub struct ChdrPacket {
    pub header: ChdrHeader,
    pub timestamp: Option<u64>,
    pub payload: Vec<u8>,
}

impl ChdrPacket {
    /// Create a new CHDR packet
    pub fn new() -> Self {
        Self {
            header: ChdrHeader::new(),
            timestamp: None,
            payload: Vec::new(),
        }
    }
    
    /// Parse from bytes
    pub fn parse(data: &[u8]) -> Result<Self, crate::Error> {
        if data.len() < 8 {
            return Err(crate::Error::ProcessingError("Packet too small".to_string()));
        }
        
        let header_bytes = [
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ];
        let header = ChdrHeader::read_from(&header_bytes)
            .ok_or_else(|| crate::Error::ProcessingError("Invalid header".to_string()))?;
        
        let mut offset = 8;
        let timestamp = if header.has_timestamp() {
            if data.len() < 16 {
                return Err(crate::Error::ProcessingError("Packet too small for timestamp".to_string()));
            }
            let ts = u64::from_le_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15],
            ]);
            offset = 16;
            Some(ts)
        } else {
            None
        };
        
        let payload = data[offset..].to_vec();
        
        Ok(Self {
            header,
            timestamp,
            payload,
        })
    }
    
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.header.as_bytes());
        
        if let Some(ts) = self.timestamp {
            bytes.extend_from_slice(&ts.to_le_bytes());
        }
        
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

impl Default for ChdrPacket {
    fn default() -> Self {
        Self::new()
    }
}