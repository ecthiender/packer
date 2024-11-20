use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{self, Context};

use crate::backend::{AsHeader, PackerBackend};

/// Read in 8KB of buffer for efficient reading, for large files.
const READ_BUFFER_SIZE: usize = 8192;

pub fn unpack<T: PackerBackend>(
    packer: &T,
    input_path: PathBuf,
    output_path: PathBuf,
) -> anyhow::Result<()> {
    // 1. file open and start reading the binary file
    let archive_file = File::open(input_path)?;
    let mut reader = BufReader::new(archive_file);

    packer.read_prologue(&mut reader)?;

    let mut header_buffer = vec![0u8; packer.header_block_size()];
    loop {
        // 2. read first `block_size` bytes; this is the header
        // println!("Reading {} bytes as header", packer.header_block_size());
        reader
            .read_exact(&mut header_buffer)
            .with_context(|| "Reading header")?;

        // we have reached the EOF marker. We are done processing the tar archive.
        if packer.is_eoa(&mut reader, &header_buffer) {
            // if we see 512 bytes with 0s, read another 512 bytes block and
            // they should also be 0s to ensure we have reached EOF.
            // println!(">>EOA<<");
            break;
        }
        // println!("Processing this file..");
        process_file(packer, &mut reader, &header_buffer, &output_path)?;
        // println!("Processing this file...DONE...");
    }
    Ok(())
}

fn process_file<T: PackerBackend>(
    packer: &T,
    reader: &mut BufReader<File>,
    header_buffer: &[u8],
    output_path: &Path,
) -> anyhow::Result<()> {
    // 3. deserialize into header, this gives all the file metadata.
    let header = packer.unpack_header(reader, header_buffer)?;

    // 4. parse path to check if this directory; if yes you get a list of dirs and a filepath,
    // otherwise only a filepath
    println!("Parsed header for file : {:?}", header.get_file_name());
    let (filename, parent_dirs) = parse_path(header.get_file_name())?;
    println!(
        "Parsed path and parent dirs : {} - {}",
        filename.display(),
        parent_dirs.display()
    );

    // 5. if dir, create all empty dirs, in the correct path location
    let final_path;
    if !parent_dirs.as_os_str().is_empty() {
        final_path = output_path.join(parent_dirs);
        fs::create_dir_all(&final_path)?;
    } else {
        final_path = output_path.to_path_buf();
    }
    println!("Writing file to path: {:?} {:?}", filename, final_path);

    // 6. create an empty file with the above metadata, in the correct path location
    let filepath = final_path.join(filename);
    println!("Effective destination file path: {}", final_path.display());
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(filepath)?;
    let mut writer = BufWriter::new(file);
    let file_size = header.get_file_size();
    println!("File size {}.", file_size);

    // 8. read X number of bytes given by file size in metadata
    // 9. write those bytes into file created in 7.
    // if file is smaller than `READ_BUFFER_SIZE`, read it all at one go.
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
        writer.write_all(&buffer)?;
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
                writer.write_all(&final_buffer)?;
                total_bytes_read += bytes_remaining;
                bytes_remaining = 0;
            // if remaining bytes is more than chunk size, read in chunk size
            } else {
                reader.read_exact(&mut buffer)?;
                writer.write_all(&buffer)?;
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

/// Takes a path, returns the filename and any parent directories. For example, given
/// `/some/path/foo/bar.txt`, this returns `(bar.txt, /some/path/foo)`.
fn parse_path(path: &Path) -> anyhow::Result<(PathBuf, PathBuf)> {
    let filename = path
        .file_name()
        .map(|os_str| Path::new(os_str).to_path_buf())
        .with_context(|| "Unable to get filename from path")?;
    let mut ancestors = path.ancestors().map(|a| a.to_owned()).collect::<Vec<_>>();
    let dirs_path = if ancestors.len() < 2 {
        PathBuf::new()
    } else {
        ancestors.swap_remove(1)
    };
    Ok((filename, dirs_path))
}
