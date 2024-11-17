//! This module contains functions to convert Rust values (mostly primitive values) into byte
//! arrays. This is used for binary serialization/deserialization.

use std::path::PathBuf;
use std::str;

pub fn u32_to_bytes(value: u32) -> [u8; 4] {
    value.to_le_bytes() // Convert the u32 to a 4-byte array (little-endian)
}

pub fn bytes_to_u32(input: [u8; 4]) -> u32 {
    u32::from_le_bytes(input) // Convert to u32 in little-endian order
}

pub fn u64_to_bytes(value: u64) -> [u8; 8] {
    value.to_le_bytes() // Convert the u64 to a 8-byte array (little-endian)
}

pub fn bytes_to_u64(input: [u8; 8]) -> u64 {
    u64::from_le_bytes(input) // Convert to u64 in little-endian order
}

pub fn i64_to_bytes(value: i64) -> [u8; 8] {
    value.to_le_bytes() // Convert the i64 to a 8-byte array (little-endian)
}

pub fn bytes_to_i64(input: [u8; 8]) -> i64 {
    i64::from_le_bytes(input) // Convert to i64 in little-endian order
}

pub fn path_to_bytes(path: PathBuf) -> Vec<u8> {
    path.to_str()
        .map(|path_str| path_str.as_bytes().to_vec())
        .unwrap_or_default()
}

pub fn bytes_to_path(array: &[u8]) -> PathBuf {
    // Find the first null terminator (0u8) to handle null-padded strings
    // let valid_length = array.iter().position(|&byte| byte == 0).unwrap_or(0);

    // Convert the valid part of the byte array to a UTF-8 string
    let path_str = str::from_utf8(array).unwrap_or("");

    // Convert the string to a PathBuf
    PathBuf::from(path_str)
}

//fn to_bytes<T: ToBytes, const N: usize>(value: T) -> [u8; N] {
//    let mut padded = [0u8; N]; // Create an N-byte array initialized with zeros
//    let bytes = value.to_le_bytes(); // Convert the T to a N-byte array (little-endian)
//    padded[..N].copy_from_slice(&bytes); // Copy the N bytes into the first half
//    padded
//}
