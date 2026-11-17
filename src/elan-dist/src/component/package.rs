//! An interpreter for the lean-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

use crate::errors::*;

use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};

use time::OffsetDateTime;
use zip::ZipArchive;

#[derive(Debug)]
pub struct TarPackage();

impl TarPackage {
    pub fn unpack<R: Read>(stream: R, path: &Path) -> Result<()> {
        let mut archive = tar::Archive::new(stream);
        // The lean-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        unpack_without_first_dir(&mut archive, path)
    }
}

/// Whether `component` is a plain file name that cannot take on another meaning
/// when combined with other components, also on Windows (e.g. `C:`, `.. `).
fn is_plain_name(component: Component<'_>) -> bool {
    match component {
        Component::Normal(name) => {
            let mut parsed = Path::new(name).components();
            matches!(parsed.next(), Some(Component::Normal(_)))
                && parsed.next().is_none()
                && !name.to_string_lossy().ends_with(['.', ' '])
        }
        _ => false,
    }
}

/// Returns `path` relative to the unpacking destination, i.e. without its first
/// component, or an error if it could refer to a location outside the destination.
fn strip_first_dir(path: &Path) -> Result<PathBuf> {
    let mut components = path.components();
    // Throw away the first path component
    components.next();
    let mut relpath = PathBuf::new();
    for component in components {
        match component {
            Component::CurDir => {}
            c if is_plain_name(c) => relpath.push(c),
            _ => return Err(format!("invalid path in archive: '{}'", path.display()).into()),
        }
    }
    Ok(relpath)
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
        let stripped = strip_first_dir(&relpath)?;
        let kind = entry.header().entry_type();
        // An entry like `pkg` or `pkg/.` refers to the destination itself. Only a
        // directory may be unpacked there: anything else would be created at
        // `path`, and a symlink there would be resolved relative to the parent
        // of `path`, letting later entries escape through it.
        if stripped.as_os_str().is_empty() && !kind.is_dir() {
            return Err(format!("invalid path in archive: '{}'", relpath.display()).into());
        }
        let full_path = path.join(stripped);

        if kind.is_hard_link() {
            return Err(
                format!("unsupported hard link in archive: '{}'", relpath.display()).into(),
            );
        }
        if kind.is_symlink() {
            // Only allow links to the link's own directory or below it, so that no
            // chain of links can lead outside the destination.
            let target = entry
                .link_name()
                .chain_err(|| ErrorKind::ExtractingPackage)?;
            let is_contained = target.as_ref().is_some_and(|target| {
                target
                    .components()
                    .all(|c| c == Component::CurDir || is_plain_name(c))
            });
            if !is_contained {
                return Err(format!("invalid symlink in archive: '{}'", relpath.display()).into());
            }
        }

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
pub struct ZipPackage();

impl ZipPackage {
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
            let stripped = strip_first_dir(Path::new(entry.name()))?;
            // A file entry like `pkg` would be written at the destination itself.
            if stripped.as_os_str().is_empty() {
                return Err(format!("invalid path in archive: '{}'", entry.name()).into());
            }
            let full_path = path.join(stripped);

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
            let mtime = OffsetDateTime::try_from(entry.last_modified().unwrap_or_else(zip::DateTime::default_for_write))?.unix_timestamp_nanos();
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
pub struct TarGzPackage();

impl TarGzPackage {
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
pub struct TarZstdPackage();

impl TarZstdPackage {
    pub fn unpack<R: Read>(stream: R, path: &Path) -> Result<()> {
        let stream = zstd::stream::read::Decoder::new(stream)?;

        TarPackage::unpack(stream, path)
    }
    pub fn unpack_file(path: &Path, into: &Path) -> Result<()> {
        let file = File::open(path).chain_err(|| ErrorKind::ExtractingPackage)?;
        Self::unpack(file, into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn tar_header(kind: tar::EntryType, path: &str, size: u64) -> tar::Header {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(kind);
        // Bypass `set_path`, which refuses to write invalid paths.
        header.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
        header.set_mode(0o644);
        header.set_size(size);
        header.set_cksum();
        header
    }

    enum TarEntry<'a> {
        Dir(&'a str),
        File(&'a str),
        Symlink(&'a str, &'a str),
        Hardlink(&'a str, &'a str),
    }

    fn unpack_tar(entries: &[TarEntry<'_>]) -> (tempfile::TempDir, Result<()>) {
        let mut builder = tar::Builder::new(Vec::new());
        for entry in entries {
            match *entry {
                TarEntry::Dir(path) => {
                    let mut header = tar_header(tar::EntryType::Directory, path, 0);
                    header.set_mode(0o755);
                    header.set_cksum();
                    builder.append(&header, io::empty()).unwrap();
                }
                TarEntry::File(path) => {
                    let header = tar_header(tar::EntryType::Regular, path, 2);
                    builder.append(&header, &b"hi"[..]).unwrap();
                }
                TarEntry::Symlink(path, target) | TarEntry::Hardlink(path, target) => {
                    let kind = match *entry {
                        TarEntry::Symlink(..) => tar::EntryType::Symlink,
                        _ => tar::EntryType::Link,
                    };
                    let mut header = tar_header(kind, path, 0);
                    header.set_link_name(target).unwrap();
                    header.set_cksum();
                    builder.append(&header, io::empty()).unwrap();
                }
            }
        }
        let data = builder.into_inner().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let result = TarPackage::unpack(&data[..], &dir.path().join("dest"));
        (dir, result)
    }

    fn unpack_zip(names: &[&str]) -> (tempfile::TempDir, Result<()>) {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"hi").unwrap();
        }
        let data = zip.finish().unwrap().into_inner();
        let dir = tempfile::tempdir().unwrap();
        let result = ZipPackage::unpack(Cursor::new(data), &dir.path().join("dest"));
        (dir, result)
    }

    #[test]
    fn tar_unpacks_normal_archive() {
        let (dir, result) = unpack_tar(&[
            TarEntry::Dir("pkg/"),
            TarEntry::File("pkg/bin/lean"),
            TarEntry::File("pkg/lib/libfoo.so.1"),
            TarEntry::Symlink("pkg/lib/libfoo.so", "libfoo.so.1"),
        ]);
        result.unwrap();
        let dest = dir.path().join("dest");
        assert_eq!(fs::read(dest.join("bin/lean")).unwrap(), b"hi");
        assert_eq!(fs::read(dest.join("lib/libfoo.so")).unwrap(), b"hi");
    }

    #[test]
    fn tar_rejects_parent_dir() {
        let (dir, result) = unpack_tar(&[TarEntry::File("pkg/../evil")]);
        assert!(result.is_err());
        assert!(!dir.path().join("evil").exists());
    }

    #[test]
    fn tar_rejects_trailing_dot_or_space() {
        for path in ["pkg/.. /evil", "pkg/... /evil", "pkg/evil."] {
            let (_dir, result) = unpack_tar(&[TarEntry::File(path)]);
            assert!(result.is_err(), "{}", path);
        }
    }

    #[test]
    fn tar_strips_root_dir() {
        // The leading `/` is the stripped first component, so the entry stays inside.
        let (dir, result) = unpack_tar(&[TarEntry::File("/evil")]);
        result.unwrap();
        assert!(dir.path().join("dest/evil").exists());
    }

    #[test]
    fn tar_rejects_symlink_outside() {
        for target in ["..", "../evil", "/tmp", "sub/../..", ".. "] {
            let (_dir, result) = unpack_tar(&[TarEntry::Symlink("pkg/link", target)]);
            assert!(result.is_err(), "{}", target);
        }
    }

    #[test]
    fn tar_rejects_file_through_symlink_outside() {
        let (dir, result) = unpack_tar(&[
            TarEntry::Symlink("pkg/link", ".."),
            TarEntry::File("pkg/link/evil"),
        ]);
        assert!(result.is_err());
        assert!(!dir.path().join("evil").exists());
    }

    #[test]
    fn tar_rejects_symlink_at_destination() {
        for path in ["pkg", "pkg/."] {
            let (dir, result) =
                unpack_tar(&[TarEntry::Symlink(path, "."), TarEntry::File("pkg/evil")]);
            // On Unix, `symlink` would also fail on the `dest/` path that results
            // from joining the empty path, so check that our own check fires.
            let err = result.unwrap_err().to_string();
            assert!(err.starts_with("invalid path in archive"), "{}: {}", path, err);
            assert!(!dir.path().join("evil").exists(), "{}", path);
        }
    }

    #[test]
    fn tar_rejects_file_at_destination() {
        for path in ["pkg", "pkg/."] {
            let (dir, result) = unpack_tar(&[TarEntry::File(path)]);
            let err = result.unwrap_err().to_string();
            assert!(err.starts_with("invalid path in archive"), "{}: {}", path, err);
            assert!(!dir.path().join("dest").is_file(), "{}", path);
        }
    }

    #[test]
    fn tar_rejects_hard_link() {
        // Hard link targets are not relative to the destination but to the
        // working directory, which for tests contains `Cargo.toml`.
        let (_dir, result) = unpack_tar(&[TarEntry::Hardlink("pkg/b", "Cargo.toml")]);
        assert!(result.is_err());
    }

    #[test]
    fn zip_unpacks_normal_archive() {
        let (dir, result) = unpack_zip(&["pkg/bin/lean.exe", "pkg/lib/foo.dll"]);
        result.unwrap();
        let dest = dir.path().join("dest");
        assert_eq!(fs::read(dest.join("bin/lean.exe")).unwrap(), b"hi");
        assert_eq!(fs::read(dest.join("lib/foo.dll")).unwrap(), b"hi");
    }

    #[test]
    fn zip_rejects_parent_dir() {
        for name in ["pkg/../evil", "pkg/sub/../../evil", "pkg/.. /evil"] {
            let (dir, result) = unpack_zip(&[name]);
            assert!(result.is_err(), "{}", name);
            assert!(!dir.path().join("evil").exists());
        }
    }

    #[test]
    fn zip_rejects_file_at_destination() {
        for name in ["pkg", "pkg/."] {
            let (dir, result) = unpack_zip(&[name]);
            let err = result.unwrap_err().to_string();
            assert!(err.starts_with("invalid path in archive"), "{}: {}", name, err);
            assert!(!dir.path().join("dest").is_file(), "{}", name);
        }
    }

    #[cfg(windows)]
    #[test]
    fn zip_rejects_drive_prefix() {
        for name in ["pkg/C:/evil", "pkg\\C:\\evil", "pkg/C:evil"] {
            let (_dir, result) = unpack_zip(&[name]);
            assert!(result.is_err(), "{}", name);
        }
    }
}
