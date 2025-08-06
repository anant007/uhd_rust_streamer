//! Storage backend implementation

use std::path::{Path, PathBuf};
use std::fs::{File, create_dir_all};
use std::io::{Write, BufWriter, BufReader, Read};
use async_trait::async_trait;
use tracing::info;

use crate::{Result, Error};
use crate::data_manager::capture::{PacketBuffer, FileHeader};

/// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Create a writer
    async fn create_writer(&self, filename: &str) -> Result<Box<dyn PacketWriter>>;
    /// Open a reader
    async fn open_reader(&self, filename: &str) -> Result<Box<dyn PacketReader>>;
}

/// Packet writer trait
#[async_trait]
pub trait PacketWriter: Send {
    /// Write header
    async fn write_header(&mut self, header: &FileHeader) -> Result<()>;
    /// Write packet
    async fn write_packet(&mut self, packet: &PacketBuffer) -> Result<usize>;
    /// Close writer
    async fn close(&mut self) -> Result<()>;
}

/// Packet reader trait
#[async_trait]
pub trait PacketReader: Send {
    /// Read header
    async fn read_header(&mut self) -> Result<FileHeader>;
    /// Read packet
    async fn read_packet(&mut self) -> Result<PacketBuffer>;
}

/// File-based storage backend
pub struct FileStorage {
    base_path: PathBuf,
}

impl FileStorage {
    /// Create new file storage
    pub fn new(base_path: PathBuf) -> Result<Self> {
        create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }
}

#[async_trait]
impl StorageBackend for FileStorage {
    async fn create_writer(&self, filename: &str) -> Result<Box<dyn PacketWriter>> {
        let path = self.base_path.join(filename);
        let file = File::create(path)?;
        Ok(Box::new(FileWriter {
            writer: BufWriter::new(file),
        }))
    }
    
    async fn open_reader(&self, filename: &str) -> Result<Box<dyn PacketReader>> {
        let path = self.base_path.join(filename);
        let file = File::open(path)?;
        Ok(Box::new(FileReader {
            reader: BufReader::new(file),
        }))
    }
}

/// File writer implementation
struct FileWriter {
    writer: BufWriter<File>,
}

#[async_trait]
impl PacketWriter for FileWriter {
    async fn write_header(&mut self, header: &FileHeader) -> Result<()> {
        use zerocopy::AsBytes;
        self.writer.write_all(header.as_bytes())?;
        Ok(())
    }
    
    async fn write_packet(&mut self, packet: &PacketBuffer) -> Result<usize> {
        let size = packet.data.len();
        self.writer.write_all(&(size as u32).to_le_bytes())?;
        self.writer.write_all(&packet.data)?;
        Ok(size + 4)
    }
    
    async fn close(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

/// File reader implementation
struct FileReader {
    reader: BufReader<File>,
}

#[async_trait]
impl PacketReader for FileReader {
    async fn read_header(&mut self) -> Result<FileHeader> {
        use zerocopy::FromBytes;
        let mut buffer = vec![0u8; std::mem::size_of::<FileHeader>()];
        self.reader.read_exact(&mut buffer)?;
        FileHeader::read_from(&buffer)
            .ok_or_else(|| Error::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid file header"
            )))
    }
    
    async fn read_packet(&mut self) -> Result<PacketBuffer> {
        let mut size_buf = [0u8; 4];
        self.reader.read_exact(&mut size_buf)?;
        let size = u32::from_le_bytes(size_buf) as usize;
        
        let mut data = vec![0u8; size];
        self.reader.read_exact(&mut data)?;
        
        Ok(PacketBuffer {
            stream_id: 0,
            packet_number: 0,
            timestamp: None,
            data,
        })
    }
}