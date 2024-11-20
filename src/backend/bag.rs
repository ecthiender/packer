//! This is the main module of the logical concept of the BAG archive format. This describes the
//! binary format and layout of the archive, and also provide functions to pack and unpack an
//! archive.

/*
 * Layout of the archive file -
 * --------------
 * <Global-Header>
 * <File1-Header>
 * <File1-Data>
 * <File2-Header>
 * <File2-Data>
 * ...
 * <EOA-MARKER>
 * --------------
 *
 * This is all serialized in binary. No compression is performed.
 *
 * - **Global Header** : is a structure containing information about the archive itself, version if
 * required etc. Block of 64 bytes.
 * - **File Header** : For each file to be archived, a file header structure is created containing file
 * metadata like name, size, permissions etc. Block of 64 bytes.
 * - **File data** : The file data verbatim as read from the source as byte array and written into the
 * archive.
 * - **EOA marker** : End of archive marker. A block size of 128 NULL bytes is written at the end to
 * signify EOF of the archive.
 *
 * Ordering of files do not matter. If there are nested directories present, the file name is
 * encoded with the nested path.
 *
 * See bag::header module for details about the header layout.
 */

mod byteorder;
mod global_header;
pub mod header;

use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};

use anyhow::{self, Context};

use byteorder::bytes_to_path;
use global_header::GlobalHeader;
use header::FileHeader;

use super::{AsHeader, PackerBackend};

const EOF_MARKER: [u8; 128] = [0; 128];

pub struct BagArchive;

impl BagArchive {
    pub fn new() -> Self {
        Self
    }
}

impl AsHeader for FileHeader {
    fn get_file_size(&self) -> u64 {
        self.file_size
    }

    fn get_file_name(&self) -> &PathBuf {
        &self.file_name
    }
}

impl PackerBackend for BagArchive {
    type Header = FileHeader;
    type EOAMarker = [u8; 128];

    fn write_prologue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()> {
        let header = GlobalHeader::new();
        let header_block = header.serialize()?;
        writer.write_all(&header_block)?;
        Ok(())
    }

    fn pack_header(
        &self,
        writer: &mut std::io::BufWriter<std::fs::File>,
        file: &super::FilePath,
        metadata: std::fs::Metadata,
    ) -> anyhow::Result<u64> {
        let header = FileHeader::new(&file.archive_path, metadata)?;
        let file_size = header.file_size;
        println!("Created header");
        header.pprint();
        println!("Serializing header data..");
        let header_block = header.serialize()?;
        println!("Writing header data..");
        writer.write_all(&header_block.header)?;
        println!("Writing filename and linkname..");
        writer.write_all(&header_block.file_name)?;
        Ok(file_size)
    }

    fn write_epilogue(&self, writer: &mut BufWriter<File>) -> anyhow::Result<()> {
        writer.write_all(&EOF_MARKER)?;
        Ok(())
    }

    fn read_prologue(&self, reader: &mut BufReader<File>) -> anyhow::Result<()> {
        let mut header_buffer = [0u8; 64];
        reader
            .read_exact(&mut header_buffer)
            .with_context(|| "Reading header")?;
        GlobalHeader::deserialize(&header_buffer)?;
        Ok(())
    }

    fn unpack_header(
        &self,
        reader: &mut BufReader<File>,
        header_buffer: &[u8],
    ) -> anyhow::Result<FileHeader> {
        // 3. deserialize into header
        // 4. this gives all the file metadata.
        let (mut header, filename_size) = FileHeader::deserialize(header_buffer)?;
        //println!("Parsed header: {:?}", header);
        //println!("Filename size: {:?}", filename_size);
        //println!("Link name size: {:?}", linkname_size);

        // read the variable-length filename from the archive
        let mut filename_buffer = vec![0; filename_size as usize];
        reader.read_exact(&mut filename_buffer)?;
        // println!("file name raw: {:?}", filename_buffer);
        header.file_name = bytes_to_path(&filename_buffer)?;
        // println!("parsed filename: {:?}", header.file_name);

        // read the variable-length link name from the archive
        //let mut linkname_buffer = vec![0; linkname_size as usize];
        //reader.read_exact(&mut linkname_buffer)?;
        //// println!("link name raw: {:?}", linkname_buffer);
        //let linkname = bytes_to_path(&linkname_buffer);
        //header.link_name = if linkname.as_os_str().is_empty() {
        //    None
        //} else {
        //    Some(linkname)
        //};
        // println!("parsed link name: {:?}", header.link_name);

        Ok(header)
    }

    fn is_eoa(&self, _reader: &mut BufReader<File>, header_buffer: &[u8]) -> bool {
        header_buffer == [0u8; 64]
    }

    fn header_block_size(&self) -> usize {
        64
    }
}
