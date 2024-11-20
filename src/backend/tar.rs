//! This is the main module of the logical concept of the TAR archive format. This describes the
//! binary format and layout of the archive, and also provide functions to pack and unpack an
//! archive.

mod byteorder;
mod header;

use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
};

use super::{AsHeader, PackerBackend};
use anyhow;
use header::Header;

const EOF_MARKER: [u8; 1024] = [0; 1024];

pub struct TarArchive;

impl TarArchive {
    pub fn new() -> Self {
        Self
    }
}

impl AsHeader for Header {
    fn get_file_size(&self) -> u64 {
        self.file_size
    }

    fn get_file_name(&self) -> &PathBuf {
        &self.file_name
    }
}

impl PackerBackend for TarArchive {
    type Header = Header;
    type EOAMarker = [u8; 1024];

    fn write_prologue(&self, _writer: &mut BufWriter<File>) -> anyhow::Result<()> {
        Ok(())
    }

    fn pack_header(
        &self,
        writer: &mut BufWriter<File>,
        file: &super::FilePath,
        metadata: std::fs::Metadata,
    ) -> anyhow::Result<u64> {
        let header = Header::new(&file.archive_path, metadata)?;
        let file_size = header.file_size;
        // println!("Created header: {:?}", header);
        // println!("Serializing header data..");
        let header_data = header.serialize()?;
        // println!("Writing header data..");
        writer.write_all(&header_data)?;
        Ok(file_size)
    }

    fn write_epilogue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()> {
        writer.write_all(&EOF_MARKER)?;
        Ok(())
    }

    fn read_prologue(&self, _reader: &mut BufReader<File>) -> anyhow::Result<()> {
        Ok(())
    }

    fn unpack_header(
        &self,
        _reader: &mut BufReader<File>,
        header_buffer: &[u8],
    ) -> anyhow::Result<Self::Header> {
        Header::deserialize(header_buffer)
    }

    fn is_eoa(&self, _reader: &mut BufReader<File>, header_buffer: &[u8]) -> bool {
        header_buffer == [0u8; 512]
    }

    fn header_block_size(&self) -> usize {
        512
    }
}
