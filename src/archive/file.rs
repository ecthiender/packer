//! Utility functions for buffered file operations

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::Context;

/// Read in 8KB of buffer for efficient reading, for large files.
const READ_BUFFER_SIZE: usize = 8192;

/// Read the whole file given by path, in chunks, in a buffered manner. Whenever data is obtained
/// the callback function is called.
pub fn read_file_chunked<F>(path: &Path, file_size: u64, mut callback: F) -> anyhow::Result<()>
where
    F: FnMut(&[u8]) -> anyhow::Result<()>,
{
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    if file_size < READ_BUFFER_SIZE as u64 {
        println!(
            "File size is smaller than 8KB. So creating a buffer of size: {}",
            file_size
        );
        let mut buffer = vec![0u8; file_size as usize];
        reader
            .read_exact(&mut buffer)
            .with_context(|| "Reading exact file size")?;
        callback(&buffer)?;
        println!("Called callback..");
    } else {
        let mut buffer = [0u8; READ_BUFFER_SIZE];
        let mut total_bytes_read: u64 = 0;
        while total_bytes_read < file_size {
            let bytes_read = reader.read(&mut buffer)?;
            println!("Read {} bytes of data..", bytes_read);
            if bytes_read == 0 {
                assert_eq!(total_bytes_read, file_size);
                break;
            }
            callback(&buffer[..bytes_read])?;
            println!("Called callback..");
            total_bytes_read += bytes_read as u64;
        }
        println!(
            "File size: {}. Total bytes read: {}",
            file_size, total_bytes_read
        );
    }
    Ok(())
}

/// Given a `BufReader`, read only slice of it, in chunks, in a buffered manner; till the given file
/// size is reached. Whenever data is obtained the callback function is called.
pub fn read_file_slice_chunked<F>(
    reader: &mut BufReader<File>,
    file_size: u64,
    mut callback: F,
) -> anyhow::Result<()>
where
    F: FnMut(&[u8]) -> anyhow::Result<()>,
{
    if file_size < READ_BUFFER_SIZE as u64 {
        println!(
            "File size is smaller than 8KB. So creating a buffer of size: {}",
            file_size
        );
        let mut buffer = vec![0u8; file_size as usize];
        println!("Reading actual file data and writing to destination file");
        reader
            .read_exact(&mut buffer)
            .with_context(|| "Reading exact file size")?;
        callback(&buffer)?;
        println!("Wrote data to file..");
    // if file is bigger than `READ_BUFFER_SIZE`, read it in `READ_BUFFER_SIZE` chunks
    } else {
        let mut buffer = [0u8; READ_BUFFER_SIZE];
        let mut total_bytes_read: u64 = 0;
        let mut bytes_remaining = file_size;
        println!("Reading actual file data and writing to destination file");

        while bytes_remaining > 0 {
            // if remaining bytes is smaller than chunk size, read those remaining bytes at one go.
            if bytes_remaining < READ_BUFFER_SIZE as u64 {
                let mut final_buffer = vec![0u8; bytes_remaining as usize];
                reader.read_exact(&mut final_buffer)?;
                callback(&final_buffer)?;
                total_bytes_read += bytes_remaining;
                bytes_remaining = 0;
            // if remaining bytes is more than chunk size, read in chunk size
            } else {
                reader.read_exact(&mut buffer)?;
                callback(&buffer)?;
                bytes_remaining -= READ_BUFFER_SIZE as u64;
                total_bytes_read += READ_BUFFER_SIZE as u64;
            }
        }
        println!(
            "File size: {}. Total bytes read: {}",
            file_size, total_bytes_read
        );
    }
    Ok(())
}
