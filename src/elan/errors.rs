use elan_dist::manifest::Component;
use elan_dist::{self, temp};
use elan_utils;
use std::path::PathBuf;
use toml;

error_chain! {
    links {
        Dist(elan_dist::Error, elan_dist::ErrorKind);
        Utils(elan_utils::Error, elan_utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
    }

    errors {
        UnknownMetadataVersion(v: String) {
            description("unknown metadata version")
            display("unknown metadata version: '{}'", v)
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        NoDefaultToolchain {
            description("no default toolchain configured. run `elan default stable` to install & configure the latest Lean 3 community release.")
        }
        OverrideToolchainNotInstalled(t: String) {
            description("override toolchain is not installed")
            display("override toolchain '{}' is not installed", t)
        }
        BinaryNotFound(t: String, bin: String) {
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
        ComponentsUnsupported(t: String) {
            description("toolchain does not support components")
            display("toolchain '{}' does not support components", t)
        }
        UnknownComponent(t: String, c: Component) {
            description("toolchain does not contain component")
            display("toolchain '{}' does not contain component {}", t, c.description())
        }
        AddingRequiredComponent(t: String, c: Component) {
            description("required component cannot be added")
            display("component {} was automatically added because it is required for toolchain '{}'",
                    c.description(), t)
        }
        ParsingSettings(e: toml::de::Error) {
            description("error parsing settings")
        }
        RemovingRequiredComponent(t: String, c: Component) {
            description("required component cannot be removed")
            display("component {} is required for toolchain '{}' and cannot be removed",
                    c.description(), t)
        }
        NoExeName {
            description("couldn't determine self executable name")
        }
        TelemetryCleanupError {
            description("unable to remove old telemetry files")
        }
        TelemetryAnalysisError {
            description("error analyzing telemetry files")
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
