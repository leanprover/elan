use elan_dist::dist::ToolchainDesc;
use elan_dist::manifest::Component;
use elan_dist::{self, temp};
use std::path::PathBuf;

use error_chain::error_chain;

error_chain! {
    links {
        Dist(elan_dist::Error, elan_dist::ErrorKind);
        Utils(elan_utils::Error, elan_utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
    }

    errors {
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
        UnknownMetadataVersion(v: String) {
            description("unknown metadata version")
            display("unknown metadata version: '{}'", v)
        }
        NoDefaultToolchain {
            description("no default toolchain configured. run `elan default stable` to install & configure the latest Lean 4 stable release.")
        }
        OverrideToolchainNotInstalled(t: ToolchainDesc) {
            description("override toolchain is not installed")
            display("override toolchain '{}' is not installed", t)
        }
        BinaryNotFound(t: ToolchainDesc, bin: String) {
            description("toolchain does not contain binary")
            display("toolchain '{}' does not have the binary `{}`", t, bin)
        }
        NeedMetadataUpgrade {
            description("elan's metadata is out of date. run `elan self upgrade-data`")
        }
        UpgradeIoError {
            description("I/O error during upgrade")
        }
        BadInstallerType(s: String) {
            description("invalid extension for installer")
            display("invalid extension for installer: '{}'", s)
        }
        ParsingSettings(e: toml::de::Error) {
            description("error parsing settings")
        }
        RemovingRequiredComponent(t: ToolchainDesc, c: Component) {
            description("required component cannot be removed")
            display("component {} is required for toolchain '{}' and cannot be removed",
                    c.description(), t)
        }
        NoExeName {
            description("couldn't determine self executable name")
        }
        InvalidLeanpkgFile(path: PathBuf, error: toml::de::Error) {
            description("couldn't parse 'leanpkg.toml'")
            display("couldn't parse '{}': '{}'", path.display(), error)
        }
        InvalidLeanVersion(path: PathBuf, t: &'static str) {
            description("invalid 'package.lean_version' value")
            display("invalid 'package.lean_version' value in '{}': expected string instead of {}", path.display(), t)
        }
    }
}
