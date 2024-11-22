use std::fs::File;
use std::fs::{self};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{self, bail, Context};

use crate::archive::file::read_file_chunked;
use crate::backend::{FilePath, PackerBackend};

pub fn pack<T: PackerBackend>(
    packer: &T,
    archive_path: PathBuf,
    files: &[PathBuf],
) -> anyhow::Result<()> {
    let outfile = File::create(archive_path)?;
    let mut writer = BufWriter::new(outfile);

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

    packer.write_prologue(&mut writer)?;
    process_files(packer, &mut writer, &file_defs)?;
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
    log::debug!("Processing file: {}", file_def.archive_path.display());
    // read file metadata
    let metadata = fs::symlink_metadata(&file_def.system_path).with_context(|| {
        format!(
            "Unable to read metadata of: {}",
            file_def.system_path.display()
        )
    })?;

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
    // if file is a symlink
    } else if metadata.is_symlink() {
        // To handle symlinks; two possible options -
        // ### Tar style
        // - During archive creation - it stores only the target name of the symlink and symlink
        // metadata.
        // - During extraction - it creates a symlink in destination and sets the target to stored
        // one. If the target file actually exists (and thus the symlink is invalid) in the sytem is
        // ignored.
        //
        // ### Packer style
        // - During create archive - check if symlink target file is already in archive -
        //   1. If yes, then just link to it.
        //   2. If no,then copy data of the target file into the archive.
        // - During extraction -
        //   1. case 1 -  remains as is.
        //   2. case 2 - turn this to a regular file when unpacked.
        // --------
        // Going for TAR style as the first implementation.
        dbg!(&metadata);
        let target = fs::read_link(&file_def.system_path)?;
        let _file_size = packer.pack_header(writer, file_def, metadata, Some(target))?;
    // if file is a regular file, then proceed with the base case
    } else if metadata.is_file() {
        let file_size = packer.pack_header(writer, file_def, metadata, None)?;
        // once header is packed; pack the source file into the archive.

        // log::trace!("Open file for reading data..");
        // open the current file for reading
        read_file_chunked(&file_def.system_path, file_size, |data| {
            writer.write_all(data)?;
            log::trace!("Wrote data to file..");
            Ok(())
        })?;
    } else {
        bail!("Unknown file type. Only regular files, directories and symlinks are supported.");
    }
    Ok(())
}
