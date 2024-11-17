use std::fs::File;
use std::fs::{self};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{self, Context};

use crate::backend::{FilePath, PackerBackend};

pub fn pack<T: PackerBackend>(
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
