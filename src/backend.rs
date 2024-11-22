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

#[derive(Debug)]
#[allow(dead_code)]
pub struct FileMetadata {
    pub file_name: PathBuf,
    pub file_size: u64,
    pub file_mode: u32,
    pub user_id: u32,
    pub group_id: u32,
    pub created_at: i64,
    pub last_modified: i64,
    pub link_name: Option<PathBuf>,
}

/// Indicates a specific packer backend, or in other words a different archive format. Each archive
/// format is backed by a backend implementation. Currently we support the BAG and TAR formats.
pub trait PackerBackend {
    /// The header type
    type Header: AsHeader;

    /// End of archive (EOA) marker.
    type EOAMarker;

    /** packing related functions **/

    /// Write any prologue at the begining of the archive file.
    fn write_prologue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()>;

    /// Pack a header to the writer.
    fn pack_header(
        &self,
        writer: &mut BufWriter<File>,
        file: &FilePath,
        metadata: fs::Metadata,
        // only set if the file is a symlink
        link_name: Option<PathBuf>,
    ) -> anyhow::Result<u64>;

    /// Write any epilogue at the end of the archive file. For example, this can be used to write
    /// End Of Archive (EOF) markers.
    fn write_epilogue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()>;

    /** unpacking related functions **/

    /// Read any prologue at the begining of the archive file.
    fn read_prologue(&self, reader: &mut BufReader<File>) -> anyhow::Result<()>;

    /// Unpack a header from the reader.
    fn unpack_header(
        &self,
        reader: &mut BufReader<File>,
        header_buffer: &[u8],
    ) -> anyhow::Result<Self::Header>;

    /// Check if End Of Archive (EOA) is reached
    fn is_eoa(&self, reader: &mut BufReader<File>, header_buffer: &[u8]) -> bool;

    /// Get the header block size
    fn header_block_size(&self) -> usize;
}

pub trait AsHeader {
    fn get_metadata(&self) -> FileMetadata;
}
