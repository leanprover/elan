//! An interpreter for the lean-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

extern crate filetime;
extern crate flate2;
extern crate tar;
extern crate zstd;

use errors::*;
use temp;

use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

#[derive(Debug)]
pub struct TarPackage<'a>(temp::Dir<'a>);

impl<'a> TarPackage<'a> {
    pub fn unpack<R: Read>(stream: R, path: &Path) -> Result<()> {
        let mut archive = tar::Archive::new(stream);
        // The lean-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        unpack_without_first_dir(&mut archive, path)
    }
}

fn unpack_without_first_dir<R: Read>(archive: &mut tar::Archive<R>, path: &Path) -> Result<()> {
    let entries = archive
        .entries()
        .chain_err(|| ErrorKind::ExtractingPackage)?;
    for entry in entries {
        let mut entry = entry.chain_err(|| ErrorKind::ExtractingPackage)?;
        let relpath = {
            let path = entry.path();
            let path = path.chain_err(|| ErrorKind::ExtractingPackage)?;
            path.into_owned()
        };
        let mut components = relpath.components();
        // Throw away the first path component
        components.next();
        let full_path = path.join(&components.as_path());

        // Create the full path to the entry if it does not exist already
        match full_path.parent() {
            Some(parent) if !parent.exists() => {
                ::std::fs::create_dir_all(&parent).chain_err(|| ErrorKind::ExtractingPackage)?
            }
            _ => (),
        };

        entry
            .unpack(&full_path)
            .chain_err(|| ErrorKind::ExtractingPackage)?;
    }

    Ok(())
}

#[derive(Debug)]
pub struct ZipPackage<'a>(temp::Dir<'a>);

impl<'a> ZipPackage<'a> {
    pub fn unpack<R: Read + Seek>(stream: R, path: &Path) -> Result<()> {
        let mut archive = ZipArchive::new(stream).chain_err(|| ErrorKind::ExtractingPackage)?;
        /*
        let mut src = archive.by_name("elan-init.exe").chain_err(|| "failed to extract update")?;
        let mut dst = fs::File::create(setup_path)?;
        io::copy(&mut src, &mut dst)?;
        */
        // The lean-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        Self::unpack_without_first_dir(&mut archive, &path)
    }
    pub fn unpack_file(path: &Path, into: &Path) -> Result<()> {
        let file = File::open(path).chain_err(|| ErrorKind::ExtractingPackage)?;
        Self::unpack(file, into)
    }

    fn unpack_without_first_dir<R: Read + Seek>(
        archive: &mut ZipArchive<R>,
        path: &Path,
    ) -> Result<()> {
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .chain_err(|| ErrorKind::ExtractingPackage)?;
            if entry.name().ends_with('/') {
                continue; // skip directories
            }
            let relpath = PathBuf::from(entry.name());
            let mut components = relpath.components();
            // Throw away the first path component
            components.next();
            let full_path = path.join(&components.as_path());

            // Create the full path to the entry if it does not exist already
            match full_path.parent() {
                Some(parent) if !parent.exists() => {
                    fs::create_dir_all(&parent).chain_err(|| ErrorKind::ExtractingPackage)?
                }
                _ => (),
            };

            {
                let mut dst =
                    File::create(&full_path).chain_err(|| ErrorKind::ExtractingPackage)?;
                io::copy(&mut entry, &mut dst).chain_err(|| ErrorKind::ExtractingPackage)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;

                    if let Some(mode) = entry.unix_mode() {
                        let mut ro_mode = fs::Permissions::from_mode(mode);
                        ro_mode.set_readonly(true);
                        fs::set_permissions(&full_path, ro_mode).unwrap();
                    }
                }
            } // make sure to close `dst` before setting mtime
            let mtime = entry.last_modified().to_time()?.unix_timestamp_nanos();
            let mtime = filetime::FileTime::from_unix_time(
                (mtime / 1000000000) as i64,
                (mtime % 1000000000) as u32,
            );
            filetime::set_file_times(&full_path, mtime, mtime).unwrap();
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct TarGzPackage<'a>(TarPackage<'a>);

impl<'a> TarGzPackage<'a> {
    pub fn unpack<R: Read>(stream: R, path: &Path) -> Result<()> {
        let stream = flate2::read::GzDecoder::new(stream);

        TarPackage::unpack(stream, path)
    }
    pub fn unpack_file(path: &Path, into: &Path) -> Result<()> {
        let file = File::open(path).chain_err(|| ErrorKind::ExtractingPackage)?;
        Self::unpack(file, into)
    }
}

#[derive(Debug)]
pub struct TarZstdPackage<'a>(TarPackage<'a>);

impl<'a> TarZstdPackage<'a> {
    pub fn unpack<R: Read>(stream: R, path: &Path) -> Result<()> {
        let stream = zstd::stream::read::Decoder::new(stream)?;

        TarPackage::unpack(stream, path)
    }
    pub fn unpack_file(path: &Path, into: &Path) -> Result<()> {
        let file = File::open(path).chain_err(|| ErrorKind::ExtractingPackage)?;
        Self::unpack(file, into)
    }
}
