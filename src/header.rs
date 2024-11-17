use std::io::Write;
use std::{fs, os::unix::fs::MetadataExt, path::Path};

use anyhow::anyhow;
use anyhow::bail;
use crc_any::CRCu32;

use crate::utils::{i64_to_u8_array, path_to_u8_array, u32_to_u8_array, u64_to_u8_array};

#[derive(Debug)]
pub struct Header {
    pub file_name: [u8; 100],
    pub file_mode: [u8; 8],
    pub user_id: [u8; 8],
    pub group_id: [u8; 8],
    pub file_size: [u8; 12],
    pub last_modified: [u8; 12],
    pub checksum: [u8; 8],
    pub type_flag: TypeFlag,
    pub link_name: [u8; 100],
}

#[derive(Debug)]
pub enum TypeFlag {
    Regular,
    HardLink,
    SymLink,
}

impl Header {
    pub fn new(file_name: &Path, metadata: fs::Metadata) -> anyhow::Result<Self> {
        let file_name = file_name;
        let file_mode = metadata.mode();
        let user_id = metadata.uid();
        let group_id = metadata.gid();
        let file_size = metadata.len();
        let mtime = metadata.mtime();
        let type_flag = TypeFlag::new(metadata);
        //println!("File metadata");
        //println!("-------------");
        //println!(">> File name: {:?}", file_name.file_name());
        //println!(">> File mode: {:?}", file_mode);
        //println!(">> uid: {:?}", user_id);
        //println!(">> gid: {:?}", group_id);
        //println!(">> file size: {:?}", file_size);
        //println!(">> mtime: {:?}", mtime);
        //println!(">> typeflag: {:?}", type_flag);
        //println!("-------------");
        Ok(Self {
            file_name: path_to_u8_array(file_name),
            file_mode: u32_to_u8_array(file_mode),
            user_id: u32_to_u8_array(user_id),
            group_id: u32_to_u8_array(group_id),
            file_size: u64_to_u8_array(file_size),
            last_modified: i64_to_u8_array(mtime),
            checksum: [0u8; 8],
            type_flag,
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
        self.checksum = u32_to_u8_array(checksum);
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
        buffer.write_all(&[self.type_flag.as_byte()])?;
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
        let type_flag_byte = bytes[156];
        let type_flag = TypeFlag::from_byte(type_flag_byte)?;
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
