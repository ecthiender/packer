use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{self, Context};
use filetime::FileTime;
use nix::unistd;

use crate::archive::file::read_file_slice_chunked;
use crate::backend::{AsHeader, PackerBackend};

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
        log::trace!("Reading {} bytes as header", packer.header_block_size());
        reader
            .read_exact(&mut header_buffer)
            .with_context(|| "Reading header")?;

        // we have reached the EOF marker. We are done processing the tar archive.
        if packer.is_eoa(&mut reader, &header_buffer) {
            // if we see 512 bytes with 0s, read another 512 bytes block and
            // they should also be 0s to ensure we have reached EOF.
            // log::trace!(">>EOA<<");
            break;
        }
        process_file(packer, &mut reader, &header_buffer, &output_path)?;
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
    let metadata = header.get_metadata();

    // 4. parse path to check if this directory; if yes you get a list of dirs and a filepath,
    // otherwise only a filepath
    log::trace!("Parsed header for file : {:?}", metadata.file_name);
    let (filename, parent_dirs) = parse_path(&metadata.file_name)?;
    log::trace!(
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
    log::debug!(
        "Writing file {} to path: {}",
        filename.display(),
        final_path.display()
    );

    // 6. create an empty file with the above metadata, in the correct path location
    let filepath = final_path.join(filename);
    log::trace!("Effective destination file path: {}", filepath.display());
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&filepath)?;
    let mut writer = BufWriter::new(file);
    log::trace!("File size {}.", metadata.file_size);

    // 7.1. if file is a symlink, set up a symlink
    if let Some(link_name) = metadata.link_name {
        if let Err(err) = create_symlink(&link_name, &filepath) {
            log::warn!(
                "Unable to set up symlink: '{} -> {}'. Error: {}",
                filepath.display(),
                link_name.display(),
                err
            );
            log::warn!("Symlink file created with invalid target.");
        }
    // 7.2. else process the file data from archive
    } else {
        // 8. read X number of bytes given by file size in metadata; write those bytes into file
        // created in 6.
        read_file_slice_chunked(reader, metadata.file_size, |data| {
            writer.write_all(data)?;
            Ok(())
        })?;
    }

    // 9. set file metadata
    // Set permissions
    let mut permissions = fs::metadata(&filepath)?.permissions();
    permissions.set_mode(metadata.file_mode);
    fs::set_permissions(&filepath, permissions)?;

    // Set UID and GID
    let uid = unistd::Uid::from_raw(metadata.user_id); // Replace with desired UID
    let gid = unistd::Gid::from_raw(metadata.group_id); // Replace with desired GID
    unistd::chown(&filepath, Some(uid), Some(gid)).with_context(|| "Failed to change ownership")?;

    // Set created and modification times
    let created_time = FileTime::from_unix_time(metadata.created_at, 0);
    let modified_time = FileTime::from_unix_time(metadata.last_modified, 0);
    filetime::set_file_times(filepath, created_time, modified_time)?;
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

#[cfg(unix)]
fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> anyhow::Result<()> {
    std::os::unix::fs::symlink(original, link)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> anyhow::Result<()> {
    std::os::windows::fs::symlink_file(original, link)?;
    Ok(())
}
