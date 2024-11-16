mod header;
mod utils;

use anyhow;
use header::Header;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::{fs, path::Path};

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

fn main() -> anyhow::Result<()> {
    let input_files: Vec<PathBuf> = ["ws/hello.txt", "ws/random.md", "ws/foobar"]
        .into_iter()
        .map(|fp| PathBuf::from(fp))
        .collect();
    let output_file = "ws/tarballs/output.mytar";
    println!(
        "Going to create tarball {:?} for files: {:?}",
        output_file, input_files
    );
    create_tar(PathBuf::from(output_file), &input_files)?;
    println!("Done.");
    Ok(())
}

fn create_tar(archive_path: PathBuf, files: &[PathBuf]) -> anyhow::Result<()> {
    let outfile = File::create(archive_path)?;
    let mut writer = BufWriter::new(outfile);
    process_files(&mut writer, files)?;
    println!("Finished processing and writing all files.");
    println!("Writing EOF marker now..");
    // write the EOF marker
    writer.write_all(&EOF_MARKER)?;
    Ok(())
}

fn process_files(writer: &mut BufWriter<File>, filepaths: &[PathBuf]) -> anyhow::Result<()> {
    for filepath in filepaths {
        process_file(writer, filepath)?;
    }
    Ok(())
}

fn process_file(writer: &mut BufWriter<File>, filepath: &Path) -> anyhow::Result<()> {
    println!("");
    println!("Processing file: {:?}", filepath);
    // read file metadata
    let metadata = fs::metadata(filepath)?;

    // if the file is a directory, get the top-level files, and recursively
    // process those files.
    if metadata.is_dir() {
        let mut sub_paths: Vec<PathBuf> = vec![];
        for entry in fs::read_dir(filepath)? {
            let entry = entry?;
            let path = entry.path().to_owned();
            sub_paths.push(path.into());
        }
        process_files(writer, &sub_paths)?;
    // if file is a regular file, then proceed with the base case
    } else {
        process_regular_file(writer, filepath, metadata)?;
    }

    Ok(())
}

fn process_regular_file(
    writer: &mut BufWriter<File>,
    filepath: &Path,
    metadata: fs::Metadata,
) -> anyhow::Result<()> {
    let mut header = Header::new(filepath, metadata)?;
    // println!("Created header: {:?}", header);
    let checksum = header.calculate_checksum()?;
    header.set_checksum(checksum);
    println!("Calculated header checksum: {:?}", checksum);
    println!("Serializing header data..");
    let header_data = header.serialize()?;
    println!("Writing header data..");
    writer.write_all(&header_data)?;

    println!("Open file for reading data..");
    // open the current file for reading
    let file = File::open(filepath)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192]; // 8 KB buffer for efficient reading
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        println!("Read {} bytes of data..", bytes_read);
        if bytes_read == 0 {
            break;
        }
        writer.write_all(&buffer)?;
    }

    Ok(())
}
