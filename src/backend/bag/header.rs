//! All header definitions for the BAG archive format.

/*
 * Layout of the file header -
 *
 * --------------------+---------------+--------+----------------------------------------------------+
 * | Field             |  Size(bytes)  | Offset |  Remarks                                           |
 * +-------------------+---------------+--------+----------------------------------------------------+
 * | <file-name-size>  |  8            |  0     |  Size of the file name itself                      |
 * | <file-size>       |  8            |  8     |  Size of the file                                  |
 * | <file-mode>       |  4            |  16    |  File permissions or mode                          |
 * | <uid>             |  4            |  20    |  uid of the file owner                             |
 * | <gid>             |  4            |  24    |  gid of the file group                             |
 * | <ctime>           |  8            |  28    |  Created time of file                              |
 * | <mtime>           |  8            |  36    |  Last modified time of file                        |
 * | <type-flag>       |  1            |  44    |  Flag indicating file type                         |
 * | <link-name-size>  |  8            |  45    |  Link name if file is symlink                      |
 * | <checksum>        |  4            |  53    |  Checksum of this header, with null checksum field |
 * +-------------------+---------------+--------+----------------------------------------------------+
 *
 * This header data is of 57 bytes. But a header block is treated as 64 bytes block. After 57 bytes,
 * the block is padded with 0. Headers should be written and read as this block of 64 bytes.
 *
 * Layout of file header, file name and file data -
 * --------------
 * <file-header> - 64 bytes
 * <file-name> - n bytes
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
pub struct HeaderBlock {
    pub(crate) header: [u8; 64],
    pub(crate) file_name: Vec<u8>,
    pub(crate) link_name: Vec<u8>,
}

/// The binary layout of the File Header. This is what is actually stored in the archive.
#[derive(Debug)]
struct FileHeaderLL {
    file_name: Vec<u8>,
    file_name_size: [u8; 8],
    file_size: [u8; 8],
    file_mode: [u8; 4],
    user_id: [u8; 4],
    group_id: [u8; 4],
    created_at: [u8; 8],
    last_modified: [u8; 8],
    type_flag: u8,
    link_name: Vec<u8>,
    link_name_size: [u8; 8],
    checksum: [u8; 4],
}

impl FileHeaderLL {
    fn new(header: FileHeader) -> anyhow::Result<Self> {
        let file_name_bytes = path_to_bytes(header.file_name)?;
        let file_name_size: u64 = safe_usize_to_u64(file_name_bytes.len())?;
        log::trace!(
            ">>>> File name: {:?}; file name size: {:?}",
            file_name_bytes,
            file_name_size
        );
        let (link_name_bytes, link_name_size) = header
            .link_name
            .map(|link_name| {
                let link_name_bytes = path_to_bytes(link_name)?;
                let link_name_size = safe_usize_to_u64(link_name_bytes.len())?;
                Ok::<_, anyhow::Error>((link_name_bytes, link_name_size))
            })
            .transpose()?
            .unwrap_or_default();
        log::trace!(
            ">>>> Link name: {:?}; link name size: {:?}",
            link_name_bytes,
            link_name_size
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
            type_flag: header.type_flag as u8,
            link_name: link_name_bytes,
            link_name_size: u64_to_bytes(link_name_size),
            checksum: [0u8; 4],
        })
    }

    /// calculate the checksum of this header; this assumes the checksum field is set to 0
    fn calculate_checksum(&self) -> anyhow::Result<u32> {
        let mut crc = CRCu32::crc32();
        let serialized = self.to_bytes()?;
        crc.digest(&serialized);
        Ok(crc.get_crc())
    }

    fn set_checksum(&mut self, checksum: u32) {
        self.checksum = u32_to_bytes(checksum);
    }

    /// Serialize the header into a 64 bytes block byte array.
    fn serialize(self) -> anyhow::Result<HeaderBlock> {
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

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
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

fn safe_usize_to_u64(value: usize) -> anyhow::Result<u64> {
    if value > u64::MAX as usize {
        Err(anyhow!("Value exceeds u64 maximum limit"))
    } else {
        Ok(value as u64)
    }
}

/// A high-level representation of the FileHeader which can be used by the other parts of the
/// program.
#[derive(Debug, Clone)]
pub struct FileHeader {
    pub(crate) file_name: PathBuf,
    pub(crate) file_size: u64,
    pub(crate) file_mode: u32,
    pub(crate) user_id: u32,
    pub(crate) group_id: u32,
    pub(crate) created_at: i64,
    pub(crate) last_modified: i64,
    pub(crate) type_flag: TypeFlag,
    pub(crate) link_name: Option<PathBuf>,
}

impl FileHeader {
    pub(crate) fn new(
        file_name: &Path,
        metadata: fs::Metadata,
        link_name: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
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
            link_name,
        })
    }

    pub(crate) fn pprint(&self) {
        log::debug!("File metadata");
        log::debug!("-------------");
        log::debug!(">> name: {}", self.file_name.display());
        log::debug!(">> size: {}", self.file_size);
        log::debug!(">> mode: {}", self.file_mode);
        log::debug!(">> uid: {}", self.user_id);
        log::debug!(">> gid: {}", self.group_id);
        log::debug!(">> ctime: {}", self.created_at);
        log::debug!(">> mtime: {}", self.last_modified);
        log::debug!(">> typeflag: {:?}", self.type_flag);
        log::debug!(
            ">> link name: {}",
            self.link_name
                .as_ref()
                .map(|ln| ln.display().to_string())
                .unwrap_or("<N/A>".to_string())
        );
        log::debug!("-------------");
    }

    pub(crate) fn serialize(self) -> anyhow::Result<HeaderBlock> {
        let mut header_ll = FileHeaderLL::new(self)?;
        // log::trace!("Constructed raw header: {:?}", header_ll);
        let checksum = header_ll.calculate_checksum()?;
        // log::debug!("Calculated checksum: {}", checksum);
        header_ll.set_checksum(checksum);
        // log::trace!("Constructed raw header: {:?}", header_ll);
        header_ll.serialize()
    }

    pub(crate) fn deserialize(bytes: &[u8]) -> anyhow::Result<(Self, u64, u64)> {
        let mut ll = FileHeaderLL::from_bytes(bytes)?;
        log::trace!("Low-level file header : {:?}", ll);
        // get the stored checksum
        let stored_checksum = bytes_to_u32(ll.checksum);
        // set the checksum to empty in low-level header object
        ll.set_checksum(0);
        // now calculate the checksum of deserialized header
        let calc_checksum = ll.calculate_checksum()?;
        // check if checksum matches
        if calc_checksum != stored_checksum {
            bail!(
                "Checksums don't match for file {}. This means that the BAG archive has corrupted data. Stored checksum: {}, calculated checksum: {}",
                bytes_to_path(&ll.file_name)?.display(),
                stored_checksum,
                calc_checksum
            )
        }
        let type_flag = TypeFlag::from_byte(ll.type_flag)?;
        let header = Self {
            file_name: bytes_to_path(&ll.file_name)?,
            file_size: bytes_to_u64(ll.file_size),
            file_mode: bytes_to_u32(ll.file_mode),
            user_id: bytes_to_u32(ll.user_id),
            group_id: bytes_to_u32(ll.group_id),
            created_at: bytes_to_i64(ll.created_at),
            last_modified: bytes_to_i64(ll.last_modified),
            type_flag,
            link_name: None,
        };
        Ok((
            header,
            bytes_to_u64(ll.file_name_size),
            bytes_to_u64(ll.link_name_size),
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum TypeFlag {
    Regular = 0,
    HardLink = 1,
    SymLink = 2,
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

    fn from_byte(byte: u8) -> anyhow::Result<Self> {
        match byte {
            b'0' | 0 => Ok(TypeFlag::Regular),
            b'1' | 1 => Ok(TypeFlag::HardLink),
            b'2' | 2 => Ok(TypeFlag::SymLink),
            _ => Err(anyhow!("Invalid typeflag byte: {:?}", byte)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use anyhow::Context;
    use fs::File;

    use super::*;

    fn read_typeflag(file: &mut File) -> anyhow::Result<TypeFlag> {
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf)?;
        TypeFlag::from_byte(buf[0])
    }

    #[test]
    fn test_typeflag() -> anyhow::Result<()> {
        fn serialize() -> anyhow::Result<()> {
            let mut file = File::create("/tmp/packer_bag_header_typeflag")?;
            let tf1 = TypeFlag::Regular;
            let tf2 = TypeFlag::HardLink;
            let tf3 = TypeFlag::SymLink;
            file.write_all(&[tf1 as u8])?;
            file.write_all(&[tf2 as u8])?;
            file.write_all(&[tf3 as u8])?;
            file.flush()?;
            file.sync_all()?;
            Ok(())
        }
        serialize()?;
        let mut file = File::open("/tmp/packer_bag_header_typeflag")?;

        let tf1 = read_typeflag(&mut file)?;
        assert_eq!(tf1, TypeFlag::Regular);

        let tf2 = read_typeflag(&mut file)?;
        assert_eq!(tf2, TypeFlag::HardLink);

        let tf3 = read_typeflag(&mut file)?;
        assert_eq!(tf3, TypeFlag::SymLink);
        Ok(())
    }

    #[test]
    fn test_file_header_serialization_deserialization() -> anyhow::Result<()> {
        // Create a sample FileHeader
        let file_name = PathBuf::from("test_file.txt");
        let file_size = 1024;
        let file_mode = 0o644; // Example permissions
        let user_id = 1000;
        let group_id = 1000;
        let created_at = 1633072800; // Example timestamp
        let last_modified = 1633072800; // Example timestamp
        let type_flag = TypeFlag::Regular;
        let link_name: Option<PathBuf> = None;

        let header = FileHeader {
            file_name,
            file_size,
            file_mode,
            user_id,
            group_id,
            created_at,
            last_modified,
            type_flag,
            link_name,
        };

        // Serialize the header
        let serialized_header = header
            .clone()
            .serialize()
            .with_context(|| "Failed to serialize header")?;

        // Deserialize the header
        let (deserialized_header, _, _) = FileHeader::deserialize(&serialized_header.header)?;
        // Assert that the original and deserialized headers are equal
        assert_eq!(header.file_size, deserialized_header.file_size);
        assert_eq!(header.file_mode, deserialized_header.file_mode);
        assert_eq!(header.user_id, deserialized_header.user_id);
        assert_eq!(header.group_id, deserialized_header.group_id);
        assert_eq!(header.created_at, deserialized_header.created_at);
        assert_eq!(header.last_modified, deserialized_header.last_modified);
        assert_eq!(header.type_flag, deserialized_header.type_flag);
        Ok(())
    }
}
