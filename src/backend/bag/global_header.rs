use std::io::Write;

use anyhow::anyhow;
use anyhow::bail;

#[derive(Debug)]
pub struct GlobalHeader {
    /// A static string. Always: "BAG Archive Format. By Packer. (c) Anon Ray."
    preamble: &'static str,
    /// Version of the format used. Reserved for future changes.
    version: FormatVersion,
}

const PREAMBLE: &str = "BAG AF.";

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
    /// A static string. Always: "BAG AF."
    preamble: [u8; 7],
    /// Version of the format used. Reserved for future changes.
    version: u8,
}

impl GlobalHeaderLL {
    pub fn new(header: GlobalHeader) -> Self {
        let mut buffer = [0u8; 7]; // Initialize a fixed-size array with zeros
        let bytes = header.preamble.as_bytes();
        let len = bytes.len().min(7); // Determine how much to copy
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
        buffer[..8].copy_from_slice(&data_buffer);

        Ok(buffer)
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != 64 {
            bail!("Invalid header block length: {}; expected 64.", bytes.len());
        }
        let preamble = bytes[0..7].try_into().unwrap();
        let version = bytes[7];
        Ok(Self { preamble, version })
    }
}
