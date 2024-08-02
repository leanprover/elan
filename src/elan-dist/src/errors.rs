use elan_utils;
use manifest::Component;
use std::io::{self, Write};
use std::path::PathBuf;
use temp;
use time::error::ComponentRange;
use toml;

error_chain! {
    links {
        Utils(elan_utils::Error, elan_utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
        Io(io::Error);
        Time(ComponentRange);
    }

    errors {
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
        ChecksumFailed {
            url: String,
            expected: String,
            calculated: String,
        } {
            description("checksum failed")
            display("checksum failed, expected: '{}', calculated: '{}'",
                    expected,
                    calculated)
        }
        ComponentConflict {
            name: String,
            path: PathBuf,
        } {
            description("conflicting component")
            display("failed to install component: '{}', detected conflict: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingFile {
            name: String,
            path: PathBuf,
        } {
            description("missing file in component")
            display("failure removing component '{}', file does not exist: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingDir {
            name: String,
            path: PathBuf,
        } {
            description("missing directory in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        CorruptComponent(name: String) {
            description("corrupt component manifest")
            display("component manifest for '{}' is corrupt", name)
        }
        ExtractingPackage {
            description("failed to extract package")
        }
        BadInstallerVersion(v: String) {
            description("unsupported installer version")
            display("unsupported installer version: {}", v)
        }
        BadInstalledMetadataVersion(v: String) {
            description("unsupported metadata version in existing installation")
            display("unsupported metadata version in existing installation: {}", v)
        }
        ComponentDirPermissionsFailed {
            description("I/O error walking directory during install")
        }
        ComponentFilePermissionsFailed {
            description("error setting file permissions during install")
        }
        ComponentDownloadFailed(c: Component) {
            description("component download failed")
            display("component download failed for {}", c.pkg)
        }
        Parsing(e: toml::de::Error) {
            description("error parsing manifest")
        }
        UnsupportedVersion(v: String) {
            description("unsupported manifest version")
            display("manifest version '{}' is not supported", v)
        }
        MissingPackageForComponent(c: Component) {
            description("missing package for component")
            display("server sent a broken manifest: missing package for component {}", c.name())
        }
        MissingPackageForRename(name: String) {
            description("missing package for the target of a rename")
            display("server sent a broken manifest: missing package for the target of a rename {}", name)
        }
        RequestedComponentsUnavailable(c: Vec<Component>) {
            description("some requested components are unavailable to download")
            display("{}", component_unavailable_msg(&c))
        }
    }
}

fn component_unavailable_msg(cs: &[Component]) -> String {
    assert!(!cs.is_empty());

    let mut buf = vec![];

    fn format_component(c: &Component) -> String {
        format!("'{}'", c.pkg)
    }

    if cs.len() == 1 {
        let _ = write!(
            buf,
            "component {} is unavailable for download",
            format_component(&cs[0])
        );
    } else {
        use itertools::Itertools;
        let mut cs_strs = cs.iter().map(|c| format!("'{}'", c.pkg));
        let cs_str = cs_strs.join(", ");
        let _ = write!(buf, "some components unavailable for download: {}", cs_str);
    }

    String::from_utf8(buf).expect("")
}
