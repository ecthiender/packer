mod archive;
mod backend;

use std::path::PathBuf;

use anyhow::{self, bail};
use clap::{Parser, Subcommand};

use backend::bag::BagArchive;
use backend::tar::TarArchive;

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
                    archive::pack(&packer, output_path, &input_files)?;
                }
                Format::Tar => {
                    let packer = TarArchive::new();
                    archive::pack(&packer, output_path, &input_files)?;
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
                    archive::unpack(&packer, input_path, output_path)?;
                }
                Format::Tar => {
                    let packer = TarArchive::new();
                    archive::unpack(&packer, input_path, output_path)?;
                }
            }
            println!("Done.");
        }
    }

    Ok(())
}
