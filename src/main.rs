mod archive;
mod backend;

use std::path::PathBuf;

use anyhow::{self, bail};
use clap::{Parser, Subcommand};
use log::info;

use backend::bag::BagArchive;
use backend::tar::TarArchive;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Pack/Unpack an archive
    #[command(subcommand)]
    command: Command,

    /// Archive format to use.
    #[arg(short, long, default_value_t, value_enum)]
    format: Format,

    /// Turn debugging information on
    #[arg(short, long, default_value_t, value_enum)]
    level: LogLevel,
}

#[derive(Subcommand)]
enum Command {
    /// Pack up files to create an archive.
    Pack {
        /// List of files (i.e. their paths) to pack up.
        #[arg(short, long, required(true), num_args(1..))]
        input_files: Vec<PathBuf>,
        /// Path to the output archive file.
        #[arg(short, long)]
        output_path: PathBuf,
    },
    /// Unpack files from an archive.
    Unpack {
        /// File path to the bag archive file.
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

#[derive(Clone, clap::ValueEnum, Default, Debug)]
enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

fn main() -> anyhow::Result<()> {
    // parse CLI arguments
    let cli = Cli::parse();

    // intialise the logger
    let mut clog = colog::default_builder();
    clog.filter(None, mk_log_level_filter(cli.level));
    clog.init();

    match cli.command {
        Command::Pack {
            input_files,
            output_path,
        } => {
            if input_files.is_empty() {
                bail!("No input files provided. Atleast one input file is required.");
            }

            info!(
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
            info!("Done.");
        }
        Command::Unpack {
            input_path,
            output_path,
        } => {
            if !input_path.is_file() {
                bail!("Input file has to be a bag archive.");
            }
            if !output_path.is_dir() {
                bail!("Output path has to be a directory where all contents of the archive will be unpacked.");
            }
            info!(
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
            info!("Done.");
        }
    }

    Ok(())
}

fn mk_log_level_filter(level: LogLevel) -> log::LevelFilter {
    match level {
        LogLevel::Error => log::LevelFilter::Error,
        LogLevel::Warn => log::LevelFilter::Warn,
        LogLevel::Info => log::LevelFilter::Info,
        LogLevel::Debug => log::LevelFilter::Debug,
        LogLevel::Trace => log::LevelFilter::Trace,
    }
}
