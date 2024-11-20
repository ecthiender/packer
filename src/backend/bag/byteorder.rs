//! This module contains functions to convert Rust values (mostly primitive values) into byte
//! arrays. This is used for binary serialization/deserialization.

use std::path::PathBuf;
use std::str;

use anyhow::anyhow;

// Convert u32 to a 4-byte array (little-endian)
pub fn u32_to_bytes(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

// Convert 4-byte array to u32 in little-endian order
pub fn bytes_to_u32(input: [u8; 4]) -> u32 {
    u32::from_le_bytes(input)
}

// Convert u64 to a 8-byte array (little-endian)
pub fn u64_to_bytes(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

// Convert 8-byte array to u64 in little-endian order
pub fn bytes_to_u64(input: [u8; 8]) -> u64 {
    u64::from_le_bytes(input)
}

pub fn i64_to_bytes(value: i64) -> [u8; 8] {
    value.to_le_bytes() // Convert the i64 to a 8-byte array (little-endian)
}

pub fn bytes_to_i64(input: [u8; 8]) -> i64 {
    i64::from_le_bytes(input) // Convert to i64 in little-endian order
}

pub fn path_to_bytes(path: PathBuf) -> anyhow::Result<Vec<u8>> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("Unable to convert path to str: {}", path.display()))?;
    let r = path_str.as_bytes();
    Ok(r.to_vec())
}

pub fn bytes_to_path(array: &[u8]) -> anyhow::Result<PathBuf> {
    let path_str = str::from_utf8(array)?;
    Ok(PathBuf::from(path_str))
}
