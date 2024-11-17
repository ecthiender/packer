use std::io::Write;
use std::path::PathBuf;
use std::{fs, os::unix::fs::MetadataExt, path::Path};

use anyhow::anyhow;
use anyhow::bail;
use crc_any::CRCu32;

use crate::byteorder::{
    bytes_to_i64, bytes_to_path, bytes_to_u32, bytes_to_u64, i64_to_bytes, path_to_bytes,
    u32_to_bytes, u64_to_bytes,
};

#[derive(Debug)]
pub struct Header {
    pub file_name: PathBuf,
    pub file_mode: u32,
    pub user_id: u32,
    pub group_id: u32,
    pub file_size: u64,
    pub last_modified: i64,
    pub type_flag: TypeFlag,
    // pub link_name: PathBuf,
}

impl Header {
    pub fn new(file_name: &Path, metadata: fs::Metadata) -> anyhow::Result<Self> {
        let file_name = file_name.to_owned();
        let file_mode = metadata.mode();
        let user_id = metadata.uid();
        let group_id = metadata.gid();
        let file_size = metadata.len();
        let last_modified = metadata.mtime();
        let type_flag = TypeFlag::new(metadata);
        Ok(Self {
            file_name,
            file_mode,
            user_id,
            group_id,
            file_size,
            last_modified,
            type_flag,
            // TODO: handle links
            // link_name: PathBuf::new(),
        })
    }

    #[allow(dead_code)]
    pub fn pprint(&self) {
        println!("File metadata");
        println!("-------------");
        println!(">> File name: {}", self.file_name.display());
        println!(">> File mode: {}", self.file_mode);
        println!(">> uid: {}", self.user_id);
        println!(">> gid: {}", self.group_id);
        println!(">> file size: {}", self.file_size);
        println!(">> mtime: {}", self.last_modified);
        println!(">> typeflag: {:?}", self.type_flag);
        println!("-------------");
    }

    pub fn serialize(self) -> anyhow::Result<[u8; 512]> {
        let mut header_ll = HeaderLL::new(self)?;
        let checksum = header_ll.calculate_checksum()?;
        header_ll.set_checksum(checksum);
        header_ll.serialize()
    }

    pub fn deserialize(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut ll = HeaderLL::from_bytes(bytes)?;
        // get the stored checksum
        let stored_checksum = bytes_to_u32(ll.checksum);
        // set the checksum to empty in low-level header object
        ll.set_checksum(0);
        // now calculate the checksum of deserialized header
        let calc_checksum = ll.calculate_checksum()?;
        // check if checksum matches
        if calc_checksum != stored_checksum {
            bail!(
                "ERROR: Checksums don't match for file {}. Stored checksum: {}, calculated checksum: {}",
                bytes_to_path(&ll.file_name).display(),
                stored_checksum,
                calc_checksum
            )
        }
        Ok(Self {
            file_name: bytes_to_path(&ll.file_name),
            file_mode: bytes_to_u32(ll.file_mode),
            user_id: bytes_to_u32(ll.user_id),
            group_id: bytes_to_u32(ll.group_id),
            file_size: bytes_to_u64(ll.file_size),
            last_modified: bytes_to_i64(ll.last_modified),
            type_flag: TypeFlag::from_byte(ll.type_flag)?,
            // link_name: bytes_to_path(&ll.link_name),
        })
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

/// A low-level representation of header. All values, here, are represented as byte arrays.
#[derive(Debug)]
pub struct HeaderLL {
    pub file_name: [u8; 100],
    pub file_mode: [u8; 8],
    pub user_id: [u8; 8],
    pub group_id: [u8; 8],
    pub file_size: [u8; 12],
    pub last_modified: [u8; 12],
    pub checksum: [u8; 8],
    pub type_flag: u8,
    pub link_name: [u8; 100],
}

impl HeaderLL {
    pub fn new(header: Header) -> anyhow::Result<Self> {
        Ok(Self {
            file_name: path_to_bytes(header.file_name),
            file_mode: u32_to_bytes(header.file_mode),
            user_id: u32_to_bytes(header.user_id),
            group_id: u32_to_bytes(header.group_id),
            file_size: u64_to_bytes(header.file_size),
            last_modified: i64_to_bytes(header.last_modified),
            checksum: [0u8; 8],
            type_flag: header.type_flag.as_byte(),
            link_name: [0u8; 100],
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

    /// serialize the header into a 512 block
    pub fn serialize(self) -> anyhow::Result<[u8; 512]> {
        let mut padded = [0u8; 512];
        let bytes = self.to_bytes()?;
        padded[..257].copy_from_slice(&bytes);
        Ok(padded)
    }

    fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let mut buffer = Vec::new();
        buffer.write_all(&self.file_name)?;
        buffer.write_all(&self.file_mode)?;
        buffer.write_all(&self.user_id)?;
        buffer.write_all(&self.group_id)?;
        buffer.write_all(&self.file_size)?;
        buffer.write_all(&self.last_modified)?;
        buffer.write_all(&self.checksum)?;
        buffer.write_all(&[self.type_flag])?;
        buffer.write_all(&self.link_name)?;
        Ok(buffer)
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 512 {
            bail!(
                "Invalid byte slice length: expected 512, got {}",
                bytes.len()
            );
        }
        let file_name = bytes[0..100].try_into().unwrap();
        let file_mode = bytes[100..108].try_into().unwrap();
        let user_id = bytes[108..116].try_into().unwrap();
        let group_id = bytes[116..124].try_into().unwrap();
        let file_size = bytes[124..136].try_into().unwrap();
        let last_modified = bytes[136..148].try_into().unwrap();
        let checksum = bytes[148..156].try_into().unwrap();
        let type_flag = bytes[156];
        let link_name = bytes[157..257].try_into().unwrap();
        Ok(Self {
            file_name,
            file_mode,
            user_id,
            group_id,
            file_size,
            last_modified,
            checksum,
            type_flag,
            link_name,
        })
    }
}
