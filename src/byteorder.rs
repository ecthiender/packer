use std::path::PathBuf;
/// This module contains functions to convert Rust values (mostly primitive values) into byte arrays.
/// This is used for binary serialization/deserialization.
use std::str;

// convert a u32 value to a [u8; 8] padded with zeros if required.
pub fn u32_to_bytes(value: u32) -> [u8; 8] {
    let bytes = value.to_le_bytes(); // Convert the u32 to a 4-byte array (little-endian)
    let mut padded = [0u8; 8]; // Create an 8-byte array initialized with zeros
    padded[..4].copy_from_slice(&bytes); // Copy the 4 bytes into the first half
    padded
}

pub fn bytes_to_u32(input: [u8; 8]) -> u32 {
    let mut buffer = [0u8; 4]; // Create an 4-byte buffer
    buffer.copy_from_slice(&input[..4]); // Take the first 8 bytes of the input
    u32::from_le_bytes(buffer) // Convert to u32 in little-endian order
}

// convert a u64 value to a [u8; 12] padded with zeros if required.
pub fn u64_to_bytes(value: u64) -> [u8; 12] {
    let mut padded = [0u8; 12]; // Create an 12-byte array initialized with zeros
    let bytes = value.to_le_bytes(); // Convert the u64 to a 8-byte array (little-endian)
    padded[..8].copy_from_slice(&bytes); // Copy the 8 bytes into the first half
    padded
}

pub fn bytes_to_u64(input: [u8; 12]) -> u64 {
    let mut buffer = [0u8; 8]; // Create an 8-byte buffer
    buffer.copy_from_slice(&input[..8]); // Take the first 8 bytes of the input
    u64::from_le_bytes(buffer) // Convert to u64 in little-endian order
}

// convert a i64 value to a [u8; 12] padded with zeros if required.
pub fn i64_to_bytes(value: i64) -> [u8; 12] {
    let mut padded = [0u8; 12]; // Create an 12-byte array initialized with zeros
    let bytes = value.to_le_bytes(); // Convert the i64 to a 8-byte array (little-endian)
    padded[..8].copy_from_slice(&bytes); // Copy the 8 bytes into the first half
    padded
}

pub fn bytes_to_i64(input: [u8; 12]) -> i64 {
    let mut buffer = [0u8; 8]; // Create an 8-byte buffer
    buffer.copy_from_slice(&input[..8]); // Take the first 8 bytes of the input
    i64::from_le_bytes(buffer) // Convert to i64 in little-endian order
}

// TODO: gets the filepath upto first 100 bytes
pub fn path_to_bytes(path: PathBuf) -> [u8; 100] {
    let mut buffer = [0u8; 100]; // Create a 100-byte array initialized with zeros
    if let Some(path_str) = path.to_str() {
        let path_bytes = path_str.as_bytes();
        // Copy the bytes, truncating if the path is too long
        let len = path_bytes.len().min(100);
        buffer[..len].copy_from_slice(&path_bytes[..len]);
    }
    buffer
}

pub fn bytes_to_path(array: &[u8; 100]) -> PathBuf {
    // Find the first null terminator (0u8) to handle null-padded strings
    let valid_length = array.iter().position(|&byte| byte == 0).unwrap_or(100);

    // Convert the valid part of the byte array to a UTF-8 string
    let path_str = str::from_utf8(&array[..valid_length]).unwrap_or("");

    // Convert the string to a PathBuf
    PathBuf::from(path_str)
}

//fn to_bytes<T: ToBytes, const N: usize>(value: T) -> [u8; N] {
//    let mut padded = [0u8; N]; // Create an N-byte array initialized with zeros
//    let bytes = value.to_le_bytes(); // Convert the T to a N-byte array (little-endian)
//    padded[..N].copy_from_slice(&bytes); // Copy the N bytes into the first half
//    padded
//}
