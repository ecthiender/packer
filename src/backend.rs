//! An abstract interface for various archiving backends. Each backend supports different archive formats.

pub mod bag;
pub mod tar;

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::{fs, path::PathBuf};

/// Represent different paths that we care about
#[derive(Debug)]
pub struct FilePath {
    /// File path/name to store in the archive. This is different from the
    /// actual path of the input file in the sytem, as strip the prefix and keep
    /// only the filename as the root.
    pub archive_path: PathBuf,
    /// File path to find the file in the system, while creating the archive.
    pub system_path: PathBuf,
}

/// Indicates a specific packer backend, or in other words a different archive format. Each archive
/// format is backed by a backend implementation. Currently we support the BAG and TAR formats.
pub trait PackerBackend {
    /// The header type
    type Header: AsHeader;

    /// End of archive (EOA) marker.
    type EOAMarker;

    fn is_eoa(&self, reader: &mut BufReader<File>, header_buffer: &[u8]) -> bool;

    fn read_prologue(&self, reader: &mut BufReader<File>) -> anyhow::Result<()>;

    fn write_prologue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()>;

    /// Pack a file to the writer.
    fn pack_file(
        &self,
        writer: &mut BufWriter<File>,
        file: &FilePath,
        metadata: fs::Metadata,
    ) -> anyhow::Result<()>;

    /// Unpack a header from the reader.
    fn unpack_header(
        &self,
        reader: &mut BufReader<File>,
        header_buffer: &[u8],
    ) -> anyhow::Result<Self::Header>;

    fn write_epilogue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()>;
}

pub trait AsHeader {
    fn get_file_name(&self) -> &PathBuf;
    fn get_file_size(&self) -> u64;
}
