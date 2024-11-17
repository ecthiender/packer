mod byteorder;
mod header;

use anyhow::{self, bail, Context};
use header::Header;
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/*
 * Structure of the archive file -
 * <Header-1>
 * <Data-1>
 * <Header-2>
 * <Data-2>
 * <EOF_MARKER>
 * -------
 * Resources
 * - https://www.gnu.org/software/tar/manual/html_node/Standard.html
 * - https://www.ibm.com/docs/en/zos/3.1.0?topic=formats-tar-format-tar-archives
 * - https://docs.fileformat.com/compression/tar/
 */

// const BLOCK_SIZE: u16 = 512;
const EOF_MARKER: [u8; 1024] = [0; 1024];

/// Represent different paths that we care about
#[derive(Debug)]
struct FilePath {
    /// File path/name to store in the archive. This is different from the
    /// actual path of the input file in the sytem, as strip the prefix and keep
    /// only the filename as the root.
    archive_path: PathBuf,
    /// File path to find the file in the system, while creating the archive.
    system_path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let tarring = false;
    if tarring {
        let input_files: Vec<PathBuf> = ["ws/foobar", "ws/hello.txt", "ws/random.md"]
            .into_iter()
            .map(|fp| PathBuf::from(fp))
            .collect();
        let output_file = "ws/tarballs/output.mytar";
        println!(
            "Creating an archive at {:?}, for files: {:?}",
            output_file, input_files
        );
        create_tar(PathBuf::from(output_file), &input_files)?;
        println!("Done.");
    } else {
        let infile = "ws/tarballs/output.mytar";
        let outfile = "ws/tarballs/out/";
        let inpath = PathBuf::from(infile);
        if !inpath.is_file() {
            bail!("Input file has to be a mytar archive.")
        }
        let outpath = PathBuf::from(outfile);
        if !outpath.is_dir() {
            bail!("Output path has to be a directory where all contents of the archive will be unpacked.")
        }
        println!(
            "Unpacking archive '{}' into destination directory: {}",
            inpath.display(),
            outpath.display()
        );
        untar(inpath, outpath)?;
        println!("Done.");
    }
    Ok(())
}

fn untar(input_path: PathBuf, output_path: PathBuf) -> anyhow::Result<()> {
    // 1. file open and start reading the binary file
    let archive_file = File::open(input_path)?;
    let mut reader = BufReader::new(archive_file);
    let mut header_buffer = [0u8; 512];
    loop {
        // 2. read first 512 bytes; this is the header
        // println!("Reading 512 bytes as header");
        reader
            .read_exact(&mut header_buffer)
            .with_context(|| "Reading header")?;

        // we have reached the EOF marker. We are done processing the tar archive.
        if header_buffer == [0u8; 512] {
            // if we see 512 bytes with 0s, read another 512 bytes block and
            // they should also be 0s to ensure we have reached EOF.
            // println!("Looks like EOF");
            break;
        }
        // println!("Processing this file..");
        read_file(&header_buffer, &mut reader, &output_path)?;
        // println!("Processing this file...DONE...");
    }
    Ok(())
}

/// Read in 8KB of buffer for efficient reading, for large files.
const READ_BUFFER_SIZE: usize = 1024;

fn read_file(
    header_buffer: &[u8],
    reader: &mut BufReader<File>,
    output_path: &PathBuf,
) -> anyhow::Result<()> {
    // 3. deserialize into header
    // 4. this gives all the file metadata.
    let header = Header::deserialize(header_buffer)?;

    // 5. parse path to check if this directory; if yes you get a list of dirs and a filepath, otherwise only a filepath
    // println!("Parsed header: {:?}", header);
    let (filename, parent_dirs) = parse_path(header.file_name)?;

    // 6. if dir, create all empty dirs, in the correct path location
    let final_path;
    if !parent_dirs.as_os_str().is_empty() {
        final_path = output_path.join(parent_dirs);
        fs::create_dir_all(&final_path)?;
    } else {
        final_path = output_path.to_path_buf();
    }
    // println!("Writing file to path: {:?} {:?}", filename, final_path);

    // 7. create an empty file with the above metadata, in the correct path location
    let filepath = final_path.join(filename);
    let file = OpenOptions::new().create(true).write(true).open(filepath)?;
    let mut writer = BufWriter::new(file);
    let file_size = header.file_size;
    // println!("File size {}.", file_size);

    // 8. read X number of bytes given by file size in metadata
    // 9. write those bytes into file created in 7.
    if file_size < READ_BUFFER_SIZE as u64 {
        // println!("File size is smaller than 8KB. So creating a buffer of size: {}", file_size);
        let mut buffer = vec![0u8; file_size as usize];
        // println!("Reading actual file data and writing to destination file");
        reader
            .read_exact(&mut buffer)
            .with_context(|| "Reading exact file size")?;
        writer.write_all(&buffer)?;
        // println!("Wrote data to file..");
    } else {
        let mut buffer = [0u8; READ_BUFFER_SIZE];
        let mut total_bytes_read: u64 = 0;
        // println!("Reading actual file data and writing to destination file");
        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .with_context(|| "Iterartively reading file data")?;
            // println!("Read {} bytes of data..", bytes_read);
            if bytes_read == 0 {
                break;
            }
            total_bytes_read += bytes_read as u64;
            writer.write_all(&buffer)?;
            // println!("Wrote data to file..");
            if total_bytes_read >= file_size {
                // println!("This file size reached. Breaking...");
                break;
            }
        }
    }
    Ok(())
}

/// Takes a path, returns the filename and any parent directories.
fn parse_path(path: PathBuf) -> anyhow::Result<(PathBuf, PathBuf)> {
    let filename = path
        .file_name()
        .and_then(|os_str| Some(Path::new(os_str).to_path_buf()))
        .with_context(|| "Unable to get filename from path")?;
    let mut ancestors = path.ancestors().map(|a| a.to_owned()).collect::<Vec<_>>();
    let dirs_path;
    if ancestors.len() < 1 {
        dirs_path = PathBuf::new();
    } else {
        dirs_path = ancestors.swap_remove(1);
    }
    Ok((filename, dirs_path))
}

fn create_tar(archive_path: PathBuf, files: &[PathBuf]) -> anyhow::Result<()> {
    let outfile = File::create(archive_path)?;
    let mut writer = BufWriter::new(outfile);

    let file_defs = files
        .into_iter()
        .map(|fp| {
            let path_str = fp
                .file_name()
                .and_then(|os_str| os_str.to_str())
                .with_context(|| "Unable to get filename from path")?;
            Ok(FilePath {
                archive_path: PathBuf::from(path_str),
                system_path: fp.clone(),
            })
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    process_files(&mut writer, &file_defs)?;
    // println!("Finished processing and writing all files.");
    // println!("Writing EOF marker now..");
    // write the EOF marker
    writer.write_all(&EOF_MARKER)?;
    Ok(())
}

fn process_files(writer: &mut BufWriter<File>, filepaths: &[FilePath]) -> anyhow::Result<()> {
    for filepath in filepaths {
        process_file(writer, filepath)?;
    }
    Ok(())
}

fn process_file(writer: &mut BufWriter<File>, file_def: &FilePath) -> anyhow::Result<()> {
    // println!("");
    // println!("Processing file: {:?}", filepath);
    // read file metadata
    let metadata = fs::metadata(&file_def.system_path)?;

    // if the file is a directory, get the top-level files, and recursively
    // process those files.
    if metadata.is_dir() {
        let mut sub_paths: Vec<FilePath> = vec![];
        for entry in fs::read_dir(&file_def.system_path)? {
            let entry = entry?;
            let filename = entry
                .path()
                .file_name()
                .and_then(|os_str| Some(Path::new(os_str).to_path_buf()))
                .with_context(|| "Unable to get filename from path")?;

            let name: PathBuf = file_def.archive_path.join(filename);
            sub_paths.push(FilePath {
                archive_path: name,
                system_path: entry.path().to_owned(),
            });
        }
        process_files(writer, &sub_paths)?;
    // if file is a regular file, then proceed with the base case
    } else {
        process_regular_file(writer, file_def, metadata)?;
    }

    Ok(())
}

fn process_regular_file(
    writer: &mut BufWriter<File>,
    file_def: &FilePath,
    metadata: fs::Metadata,
) -> anyhow::Result<()> {
    let header = Header::new(&file_def.archive_path, metadata)?;
    // println!("Created header: {:?}", header);
    // println!("Serializing header data..");
    let header_data = header.serialize()?;
    // println!("Writing header data..");
    writer.write_all(&header_data)?;

    // println!("Open file for reading data..");
    // open the current file for reading
    let file = File::open(&file_def.system_path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; READ_BUFFER_SIZE]; // 8 KB buffer for efficient reading
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        // println!("Read {} bytes of data..", bytes_read);
        if bytes_read == 0 {
            break;
        }
        let data = buffer
            .into_iter()
            .take_while(|c| *c != 0u8)
            .collect::<Vec<_>>();
        writer.write_all(&data)?;
    }
    Ok(())
}
