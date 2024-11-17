//! All header definitions for the BAG archive format.

/*
 * Layout of the file header -
 *
 * --------------------+---------------+--------+----------------------------------------------------+
 * | Field             |  Size(bytes)  | Offset |  Remarks                                           |
 * +-------------------+---------------+--------+----------------------------------------------------+
 * | <file-name-size>  |  8            |  ?     |  Size of the file name itself                      |
 * | <file-size>       |  8            |  ?     |  Size of the file                                  |
 * | <file-mode>       |  4            |  ?     |  File permissions or mode                          |
 * | <uid>             |  4            |  ?     |  uid of the file owner                             |
 * | <gid>             |  4            |  ?     |  gid of the file group                             |
 * | <ctime>           |  8            |  ?     |  Created time of file                              |
 * | <mtime>           |  8            |  ?     |  Last modified time of file                        |
 * | <type-flag>       |  1            |  ?     |  Flag indicating file type                         |
 * | <link-name-size>  |  8            |  ?     |  Size of the link name                             |
 * | <checksum>        |  4            |  ?     |  Checksum of this header, with null checksum field |
 * +-------------------+---------------+--------+----------------------------------------------------+
 *
 * This header data is of 57 bytes. But a header block is treated as 64 bytes block. After 57 bytes,
 * the block is padded with 0. Headers should be written and read as this block of 64 bytes.
 *
 * Layout of file header, file name and file data -
 * --------------
 * <file-header> - 64 bytes
 * <file-name> - n bytes
 * <file-link-name> - n bytes
 * <file-data> - n bytes
 * --------------
*/

use std::io::Write;
use std::{fs, os::unix::fs::MetadataExt, path::Path, path::PathBuf};

use anyhow::anyhow;
use anyhow::bail;
use crc_any::CRCu32;

use crate::backend::bag::byteorder::{
    bytes_to_i64, bytes_to_path, bytes_to_u32, bytes_to_u64, i64_to_bytes, path_to_bytes,
    u32_to_bytes, u64_to_bytes,
};

#[derive(Debug)]
pub struct GlobalHeader {
    /// A static string. Always: "BAG Archive Format. By Packer. (c) Anon Ray."
    preamble: &'static str,
    /// Version of the format used. Reserved for future changes.
    version: FormatVersion,
}

const PREAMBLE: &str = "BAG Archive Format. By Packer. (c) Anon Ray.";

impl GlobalHeader {
    pub fn new() -> Self {
        Self {
            preamble: PREAMBLE,
            version: FormatVersion::V1,
        }
    }

    pub fn serialize(self) -> anyhow::Result<[u8; 64]> {
        let ll = GlobalHeaderLL::new(self);
        ll.to_bytes()
    }

    pub fn deserialize(bytes: &[u8]) -> anyhow::Result<()> {
        let ll = GlobalHeaderLL::from_bytes(bytes)?;
        let preamble = std::str::from_utf8(&ll.preamble)?;
        if preamble != PREAMBLE {
            bail!("Error: Not a BAG Archive format. Exiting.");
        }
        let _version = FormatVersion::from_byte(ll.version)?;
        Ok(())
    }
}

#[derive(Debug)]
enum FormatVersion {
    V1,
}

impl FormatVersion {
    fn as_byte(&self) -> u8 {
        match self {
            Self::V1 => 1,
        }
    }
    fn from_byte(byte: u8) -> anyhow::Result<Self> {
        match byte {
            b'1' | 1 => Ok(Self::V1),
            _ => Err(anyhow!("Invalid version byte: {:?}", byte)),
        }
    }
}

/// Low-level repr of the global header. It is 45 bytes. But it is padded with 0s at the end to make
/// the block size of 64 bytes. Headers are read/written as this block of 64 bytes.
#[derive(Debug)]
struct GlobalHeaderLL {
    /// A static string. Always: "BAG Archive Format. By Packer. (c) Anon Ray."
    preamble: [u8; 44],
    /// Version of the format used. Reserved for future changes.
    version: u8,
}

impl GlobalHeaderLL {
    pub fn new(header: GlobalHeader) -> Self {
        let mut buffer = [0u8; 44]; // Initialize a fixed-size array with zeros
        let bytes = header.preamble.as_bytes();
        let len = bytes.len().min(44); // Determine how much to copy
        buffer[..len].copy_from_slice(&bytes[..len]); // Copy bytes into the buffer
        Self {
            preamble: buffer,
            version: header.version.as_byte(),
        }
    }

    pub fn to_bytes(&self) -> anyhow::Result<[u8; 64]> {
        let mut data_buffer = Vec::new();
        data_buffer.write_all(&self.preamble)?;
        data_buffer.write_all(&[self.version])?;

        let mut buffer = [0u8; 64];
        buffer[..45].copy_from_slice(&data_buffer);

        Ok(buffer)
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 64 {
            bail!("Invalid header block length: {}; expected 64.", bytes.len());
        }
        let preamble = bytes[0..44].try_into().unwrap();
        let version = bytes[44];
        Ok(Self { preamble, version })
    }
}

#[derive(Debug)]
pub struct HeaderBlock {
    pub header: [u8; 64],
    pub file_name: Vec<u8>,
    pub link_name: Vec<u8>,
}

/// The binary layout of the File Header. This is what is actually stored in the archive.
#[derive(Debug)]
pub struct FileHeaderLL {
    pub file_name: Vec<u8>,
    pub file_name_size: [u8; 8],
    pub file_size: [u8; 8],
    pub file_mode: [u8; 4],
    pub user_id: [u8; 4],
    pub group_id: [u8; 4],
    pub created_at: [u8; 8],
    pub last_modified: [u8; 8],
    pub type_flag: u8,
    pub link_name: Vec<u8>,
    pub link_name_size: [u8; 8],
    pub checksum: [u8; 4],
}

impl FileHeaderLL {
    pub fn new(header: FileHeader) -> anyhow::Result<Self> {
        let file_name_bytes = path_to_bytes(header.file_name);
        let file_name_size = file_name_bytes.len() as u64;
        let link_name_bytes = header.link_name.map(path_to_bytes);
        let link_name_size = link_name_bytes
            .as_ref()
            .map(|bytes| bytes.len())
            .unwrap_or(0) as u64;
        println!(
            ">>>> File name: {:?}; file name size: {:?}",
            file_name_bytes, file_name_size
        );
        println!(
            ">>>> Link name: {:?}; link name size: {:?}",
            link_name_bytes, link_name_size
        );
        Ok(Self {
            file_name: file_name_bytes,
            file_name_size: u64_to_bytes(file_name_size),
            file_size: u64_to_bytes(header.file_size),
            file_mode: u32_to_bytes(header.file_mode),
            user_id: u32_to_bytes(header.user_id),
            group_id: u32_to_bytes(header.group_id),
            created_at: i64_to_bytes(header.created_at),
            last_modified: i64_to_bytes(header.last_modified),
            type_flag: header.type_flag.as_byte(),
            link_name: link_name_bytes.unwrap_or_default(),
            link_name_size: u64_to_bytes(link_name_size),
            checksum: [0u8; 4],
        })
    }

    /// calculate the checksum of this header; this assumes the checksum field is set to 0
    pub fn calculate_checksum(&self) -> anyhow::Result<u32> {
        let mut crc = CRCu32::crc32();
        let serialized = self.to_bytes()?;
        crc.digest(&serialized);
        Ok(crc.get_crc())
    }

    pub fn set_checksum(&mut self, checksum: u32) {
        self.checksum = u32_to_bytes(checksum);
    }

    /// Serialize the header into a 64 bytes block byte array.
    pub fn serialize(self) -> anyhow::Result<HeaderBlock> {
        let mut buffer = [0u8; 64];
        let bytes = self.to_bytes()?;
        buffer[..57].copy_from_slice(&bytes);
        Ok(HeaderBlock {
            header: buffer,
            file_name: self.file_name,
            link_name: self.link_name,
        })
    }

    fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let mut buffer = Vec::new();
        buffer.write_all(&self.file_name_size)?;
        buffer.write_all(&self.file_size)?;
        buffer.write_all(&self.file_mode)?;
        buffer.write_all(&self.user_id)?;
        buffer.write_all(&self.group_id)?;
        buffer.write_all(&self.created_at)?;
        buffer.write_all(&self.last_modified)?;
        buffer.write_all(&[self.type_flag])?;
        buffer.write_all(&self.link_name_size)?;
        buffer.write_all(&self.checksum)?;
        Ok(buffer)
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 64 {
            bail!("Invalid header block length: {}; expected 64.", bytes.len());
        }
        let file_name_size = bytes[0..8].try_into().unwrap();
        let file_size = bytes[8..16].try_into().unwrap();
        let file_mode = bytes[16..20].try_into().unwrap();
        let user_id = bytes[20..24].try_into().unwrap();
        let group_id = bytes[24..28].try_into().unwrap();
        let created_at = bytes[28..36].try_into().unwrap();
        let last_modified = bytes[36..44].try_into().unwrap();
        let type_flag = bytes[44];
        let link_name_size = bytes[45..53].try_into().unwrap();
        let checksum = bytes[53..57].try_into().unwrap();

        Ok(Self {
            file_name: Vec::new(),
            file_name_size,
            file_size,
            file_mode,
            user_id,
            group_id,
            created_at,
            last_modified,
            type_flag,
            link_name: Vec::new(),
            link_name_size,
            checksum,
        })
    }
}

/// A high-level representation of the FileHeader which can be used by the other parts of the
/// program.
#[derive(Debug)]
pub struct FileHeader {
    pub file_name: PathBuf,
    pub file_size: u64,
    pub file_mode: u32,
    pub user_id: u32,
    pub group_id: u32,
    pub created_at: i64,
    pub last_modified: i64,
    pub type_flag: TypeFlag,
    pub link_name: Option<PathBuf>,
}

impl FileHeader {
    pub fn new(file_name: &Path, metadata: fs::Metadata) -> anyhow::Result<Self> {
        let file_name = file_name.to_owned();
        let file_mode = metadata.mode();
        let user_id = metadata.uid();
        let group_id = metadata.gid();
        let file_size = metadata.len();
        let created_at = metadata.ctime();
        let last_modified = metadata.mtime();
        let type_flag = TypeFlag::new(metadata);
        Ok(Self {
            file_name,
            file_size,
            file_mode,
            user_id,
            group_id,
            created_at,
            last_modified,
            type_flag,
            // TODO: handle links
            link_name: None,
        })
    }

    #[allow(dead_code)]
    pub fn pprint(&self) {
        println!("File metadata");
        println!("-------------");
        println!(">> name: {}", self.file_name.display());
        println!(">> size: {}", self.file_size);
        println!(">> mode: {}", self.file_mode);
        println!(">> uid: {}", self.user_id);
        println!(">> gid: {}", self.group_id);
        println!(">> ctime: {}", self.created_at);
        println!(">> mtime: {}", self.last_modified);
        println!(">> typeflag: {:?}", self.type_flag);
        println!(
            ">> link name: {}",
            self.link_name
                .as_ref()
                .map(|ln| ln.display().to_string())
                .unwrap_or("<N/A>".to_string())
        );
        println!("-------------");
    }

    pub fn serialize(self) -> anyhow::Result<HeaderBlock> {
        let mut header_ll = FileHeaderLL::new(self)?;
        println!("Constructed raw header: {:?}", header_ll);
        let checksum = header_ll.calculate_checksum()?;
        println!("Calculated checksum: {}", checksum);
        header_ll.set_checksum(checksum);
        println!("Constructed raw header: {:?}", header_ll);
        header_ll.serialize()
    }

    pub fn deserialize(bytes: &[u8]) -> anyhow::Result<(Self, u64, u64)> {
        let mut ll = FileHeaderLL::from_bytes(bytes)?;
        println!("Low-level file header : {:?}", ll);
        // get the stored checksum
        let stored_checksum = bytes_to_u32(ll.checksum);
        // set the checksum to empty in low-level header object
        ll.set_checksum(0);
        // now calculate the checksum of deserialized header
        let calc_checksum = ll.calculate_checksum()?;
        // check if checksum matches
        if calc_checksum != stored_checksum {
            println!(
                "WARN: Checksums don't match for file {}. Stored checksum: {}, calculated checksum: {}",
                bytes_to_path(&ll.file_name).display(),
                stored_checksum,
                calc_checksum
            )
        }
        let header = Self {
            file_name: bytes_to_path(&ll.file_name),
            file_size: bytes_to_u64(ll.file_size),
            file_mode: bytes_to_u32(ll.file_mode),
            user_id: bytes_to_u32(ll.user_id),
            group_id: bytes_to_u32(ll.group_id),
            created_at: bytes_to_i64(ll.created_at),
            last_modified: bytes_to_i64(ll.last_modified),
            type_flag: TypeFlag::from_byte(ll.type_flag)?,
            link_name: None,
        };

        Ok((
            header,
            bytes_to_u64(ll.file_name_size),
            bytes_to_u64(ll.link_name_size),
        ))
    }
}

#[derive(Debug)]
pub enum TypeFlag {
    Regular,
    HardLink,
    SymLink,
}

impl TypeFlag {
    fn new(metadata: fs::Metadata) -> Self {
        if metadata.is_symlink() {
            TypeFlag::SymLink
        } else if metadata.is_dir() {
            TypeFlag::HardLink
        } else {
            TypeFlag::Regular
        }
    }

    fn as_byte(&self) -> u8 {
        match self {
            TypeFlag::Regular => b'0',
            TypeFlag::HardLink => b'1',
            TypeFlag::SymLink => b'2',
        }
    }

    fn from_byte(byte: u8) -> anyhow::Result<Self> {
        match byte {
            b'0' | 0 => Ok(TypeFlag::Regular),
            b'1' | 1 => Ok(TypeFlag::HardLink),
            b'2' | 2 => Ok(TypeFlag::SymLink),
            _ => Err(anyhow!("Invalid typeflag byte: {:?}", byte)),
        }
    }
}
