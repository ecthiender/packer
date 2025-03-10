# Packer

A file archiving utility, like tar and others.

Packer also defines a custom archive file format called "bag", which is much more compact than tar.

Currently supported formats are -
- bag
- tar

## Why?

This is just for fun. I wanted to play with writing a low-level tool dealing with binary data
formats. It started out with being a tar clone. But in the process I thought I could come with a
better format.

## Install

- git clone this repo
- make sure you have `rustup` installed
- make sure you have the correct rust dependencies - `rustup show` (this will install/update any
  missing dependencies)
- build it - `cargo build --release`
- cp `./target/release/packer` somewhere that's accessible in your `PATH`
- then you can run the `packer` command from anywhere

## Usage

### To create an archive

```sh
packer pack -i /some/path/dir/mydir -o myarchive.bag
```

#### Multiple files and folders

```sh
packer pack \
    -i file1.txt some/path/file2.txt some/path/mydir /some/other/path/dir/mydir2 \
    -o myarchive.bag
```

### To extract from an archive

```sh
packer unpack -i myarchive.bag -o /some/path/destination-dir
```

### Other formats

It uses the bag format by default. If you want to use a different format you can pass `--format` or `-f` -

```sh
packer pack -f tar -i /some/path/to/dir -o myarchive.tar
```

## Help

Run the help command to see all possible commands and flags. Make sure to check help of the
subcommands as well.

```sh
packer --help

packer <subcommand> --help
```
