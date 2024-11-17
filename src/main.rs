mod archive;
mod backend;
mod byteorder;
mod byteorder_padded;
mod header;

use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{self, bail, Context};
use backend::tar::TarArchive;
use clap::{Parser, Subcommand};

use backend::bag::BagArchive;
use backend::PackerBackend;
use backend::{AsHeader, FilePath};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    /// Which format to use. Default bag.
    #[arg(short, long, default_value_t, value_enum)]
    format: Format,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pack up files to create an archive.
    Pack {
        /// List of files (i.e. their paths) to pack up.
        #[arg(short, long, required(true), value_delimiter = ' ')]
        input_files: Vec<PathBuf>,
        /// Path to the output archive file.
        #[arg(short, long)]
        output_path: PathBuf,
    },
    /// Unpack all files from an archive.
    Unpack {
        /// File path to the mytar archive file.
        #[arg(short, long)]
        input_path: PathBuf,
        /// Destination directory where all of the contents will be unpacked.
        #[arg(short, long)]
        output_path: PathBuf,
    },
}

#[derive(Clone, clap::ValueEnum, Default, Debug)]
enum Format {
    #[default]
    Bag,
    Tar,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Pack {
            input_files,
            output_path,
        } => {
            if input_files.is_empty() {
                bail!("No input files provided. Atleast one input file is required.");
            }
            println!(
                "Creating an archive at {}, for files: {}",
                output_path.display(),
                input_files
                    .iter()
                    .map(|f| f.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            match cli.format {
                Format::Bag => {
                    let packer = BagArchive::new();
                    pack_archive(&packer, output_path, &input_files)?;
                }
                Format::Tar => {
                    let packer = TarArchive::new();
                    pack_archive(&packer, output_path, &input_files)?;
                }
            }
            println!("Done.");
        }
        Command::Unpack {
            input_path,
            output_path,
        } => {
            if !input_path.is_file() {
                bail!("Input file has to be a mytar archive.")
            }
            if !output_path.is_dir() {
                bail!("Output path has to be a directory where all contents of the archive will be unpacked.")
            }
            println!(
                "Unpacking archive {} into destination directory: {}",
                input_path.display(),
                output_path.display()
            );
            match cli.format {
                Format::Bag => {
                    let packer = BagArchive::new();
                    unpack_archive(&packer, input_path, output_path)?;
                }
                Format::Tar => {
                    let packer = TarArchive::new();
                    unpack_archive(&packer, input_path, output_path)?;
                }
            }
            println!("Done.");
        }
    }

    Ok(())
}

fn unpack_archive<T: PackerBackend>(
    packer: &T,
    input_path: PathBuf,
    output_path: PathBuf,
) -> anyhow::Result<()> {
    // 1. file open and start reading the binary file
    let archive_file = File::open(input_path)?;
    let mut reader = BufReader::new(archive_file);

    packer.read_prologue(&mut reader)?;

    let mut header_buffer = [0u8; 64];
    loop {
        // 2. read first 512 bytes; this is the header
        println!("Reading 64 bytes as header");
        reader
            .read_exact(&mut header_buffer)
            .with_context(|| "Reading header")?;

        // we have reached the EOF marker. We are done processing the tar archive.
        if packer.is_eoa(&mut reader, &header_buffer) {
            // if we see 512 bytes with 0s, read another 512 bytes block and
            // they should also be 0s to ensure we have reached EOF.
            println!(">>EOA<<");
            break;
        }
        println!("Processing this file..");
        read_file(packer, &mut reader, &header_buffer, &output_path)?;
        println!("Processing this file...DONE...");
    }
    Ok(())
}

/// Read in 8KB of buffer for efficient reading, for large files.
const READ_BUFFER_SIZE: usize = 8192;

fn read_file<T: PackerBackend>(
    packer: &T,
    reader: &mut BufReader<File>,
    header_buffer: &[u8],
    output_path: &Path,
) -> anyhow::Result<()> {
    // 3. deserialize into header, this gives all the file metadata.
    let header = packer.unpack_header(reader, header_buffer)?;

    // 4. parse path to check if this directory; if yes you get a list of dirs and a filepath,
    // otherwise only a filepath
    // println!("Parsed header: {:?}", header);
    let (filename, parent_dirs) = parse_path(header.get_file_name())?;

    // 5. if dir, create all empty dirs, in the correct path location
    let final_path;
    if !parent_dirs.as_os_str().is_empty() {
        final_path = output_path.join(parent_dirs);
        fs::create_dir_all(&final_path)?;
    } else {
        final_path = output_path.to_path_buf();
    }
    // println!("Writing file to path: {:?} {:?}", filename, final_path);

    // 6. create an empty file with the above metadata, in the correct path location
    let filepath = final_path.join(filename);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(filepath)?;
    let mut writer = BufWriter::new(file);
    let file_size = header.get_file_size();
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

fn pack_archive<T: PackerBackend>(
    packer: &T,
    archive_path: PathBuf,
    files: &[PathBuf],
) -> anyhow::Result<()> {
    let outfile = File::create(archive_path)?;
    let mut writer = BufWriter::new(outfile);

    packer.write_prologue(&mut writer)?;

    let file_defs = files
        .iter()
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

    process_files(packer, &mut writer, &file_defs)?;
    // println!("Finished processing and writing all files.");
    // println!("Writing EOF marker now..");
    // write the EOF marker
    packer.write_epilogue(&mut writer)?;
    Ok(())
}

fn process_files<T: PackerBackend>(
    packer: &T,
    writer: &mut BufWriter<File>,
    filepaths: &[FilePath],
) -> anyhow::Result<()> {
    for filepath in filepaths {
        process_file(packer, writer, filepath)?;
    }
    Ok(())
}

fn process_file<T: PackerBackend>(
    packer: &T,
    writer: &mut BufWriter<File>,
    file_def: &FilePath,
) -> anyhow::Result<()> {
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
                .map(|os_str| Path::new(os_str).to_path_buf())
                .with_context(|| "Unable to get filename from path")?;

            let name: PathBuf = file_def.archive_path.join(filename);
            sub_paths.push(FilePath {
                archive_path: name,
                system_path: entry.path().to_owned(),
            });
        }
        process_files(packer, writer, &sub_paths)?;
    // if file is a regular file, then proceed with the base case
    } else {
        packer.pack_file(writer, file_def, metadata)?;
    }

    Ok(())
}
