//! An interpreter for the lean-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

extern crate tar;
extern crate flate2;

use component::components::*;
use component::transaction::*;

use errors::*;
use temp;

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fmt;
use std::io::{Read,Seek,self};
use std::fs::{File,self};

use zip::ZipArchive;

/// The current metadata revision used by lean-installer
pub const INSTALLER_VERSION: &'static str = "3";
pub const VERSION_FILE: &'static str = "lean-installer-version";

pub trait Package: fmt::Debug {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool;
    fn install<'a>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'a>)
                   -> Result<Transaction<'a>>;
    fn components(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct DirectoryPackage {
    path: PathBuf,
    components: HashSet<String>,
}

impl DirectoryPackage {
    pub fn new(path: PathBuf) -> Result<Self> {
        let components = vec!["lean".to_string()].into_iter().collect();
        Ok(DirectoryPackage {
            path: path,
            components: components,
        })
    }
}

impl Package for DirectoryPackage {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.components.contains(component) ||
        if let Some(n) = short_name {
            self.components.contains(n)
        } else {
            false
        }
    }
    fn install<'a>(&self,
                   target: &Components,
                   name: &str,
                   _short_name: Option<&str>,
                   tx: Transaction<'a>)
                   -> Result<Transaction<'a>> {
        assert_eq!(name, "lean");

        let mut builder = target.add(name, tx);

        for entry in ::std::fs::read_dir(&self.path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                builder.copy_dir(entry.path().strip_prefix(&self.path).unwrap().to_path_buf(), &self.path.join(entry.path()))?
            } else {
                builder.copy_file(entry.path().strip_prefix(&self.path).unwrap().to_path_buf(), &self.path.join(entry.path()))?
            }
        }

        let tx = try!(builder.finish());

        Ok(tx)
    }

    fn components(&self) -> Vec<String> {
        self.components.iter().cloned().collect()
    }
}

#[derive(Debug)]
pub struct TarPackage<'a>(DirectoryPackage, temp::Dir<'a>);

impl<'a> TarPackage<'a> {
    pub fn new<R: Read>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let temp_dir = try!(temp_cfg.new_directory());
        let mut archive = tar::Archive::new(stream);
        // The lean-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        try!(unpack_without_first_dir(&mut archive, &*temp_dir));

        Ok(TarPackage(try!(DirectoryPackage::new(temp_dir.to_owned())), temp_dir))
    }
}

fn unpack_without_first_dir<R: Read>(archive: &mut tar::Archive<R>, path: &Path) -> Result<()> {
    let entries = try!(archive.entries().chain_err(|| ErrorKind::ExtractingPackage));
    for entry in entries {
        let mut entry = try!(entry.chain_err(|| ErrorKind::ExtractingPackage));
        let relpath = {
            let path = entry.path();
            let path = try!(path.chain_err(|| ErrorKind::ExtractingPackage));
            path.into_owned()
        };
        let mut components = relpath.components();
        // Throw away the first path component
        components.next();
        let full_path = path.join(&components.as_path());

        // Create the full path to the entry if it does not exist already
        match full_path.parent() {
            Some(parent) if !parent.exists() =>
                try!(::std::fs::create_dir_all(&parent).chain_err(|| ErrorKind::ExtractingPackage)),
            _ => (),
        };

        try!(entry.unpack(&full_path).chain_err(|| ErrorKind::ExtractingPackage));
    }

    Ok(())
}

impl<'a> Package for TarPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'b>)
                   -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}

#[derive(Debug)]
pub struct ZipPackage<'a>(DirectoryPackage, temp::Dir<'a>);

impl<'a> ZipPackage<'a> {
    pub fn new<R: Read + Seek>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let temp_dir = try!(temp_cfg.new_directory());
        let mut archive = ZipArchive::new(stream).chain_err(|| ErrorKind::ExtractingPackage)?;
        /*
                let mut src = archive.by_name("elan-init.exe").chain_err(|| "failed to extract update")?;
                let mut dst = fs::File::create(setup_path)?;
                io::copy(&mut src, &mut dst)?;
                */
        // The lean-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        try!(Self::unpack_without_first_dir(&mut archive, &*temp_dir));

        Ok(ZipPackage(try!(DirectoryPackage::new(temp_dir.to_owned())), temp_dir))
    }
    pub fn new_file(path: &Path, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let file = try!(File::open(path).chain_err(|| ErrorKind::ExtractingPackage));
        Self::new(file, temp_cfg)
    }

    fn unpack_without_first_dir<R: Read + Seek>(archive: &mut ZipArchive<R>, path: &Path) -> Result<()> {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).chain_err(|| ErrorKind::ExtractingPackage)?;
            if entry.name().ends_with('/') {
                continue // skip directories
            }
            let relpath = PathBuf::from(entry.name());
            let mut components = relpath.components();
            // Throw away the first path component
            components.next();
            let full_path = path.join(&components.as_path());

            // Create the full path to the entry if it does not exist already
            match full_path.parent() {
                Some(parent) if !parent.exists() =>
                    try!(fs::create_dir_all(&parent).chain_err(|| ErrorKind::ExtractingPackage)),
                _ => (),
            };

            let mut dst = File::create(&full_path).chain_err(|| ErrorKind::ExtractingPackage)?;
            io::copy(&mut entry, &mut dst).chain_err(|| ErrorKind::ExtractingPackage)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                if let Some(mode) = entry.unix_mode() {
                    fs::set_permissions(&full_path, fs::Permissions::from_mode(mode)).unwrap();
                }
            }
        }

        Ok(())
    }
}

impl<'a> Package for ZipPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'b>)
                   -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}

#[derive(Debug)]
pub struct TarGzPackage<'a>(TarPackage<'a>);

impl<'a> TarGzPackage<'a> {
    pub fn new<R: Read>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let stream = flate2::read::GzDecoder::new(stream);

        Ok(TarGzPackage(try!(TarPackage::new(stream, temp_cfg))))
    }
    pub fn new_file(path: &Path, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let file = try!(File::open(path).chain_err(|| ErrorKind::ExtractingPackage));
        Self::new(file, temp_cfg)
    }
}

impl<'a> Package for TarGzPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'b>)
                   -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}
