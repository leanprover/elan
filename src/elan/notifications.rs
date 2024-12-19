use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use crate::errors::*;
use elan_dist::dist::ToolchainDesc;

use elan_dist::{self, temp};
use elan_utils::notify::NotificationLevel;

#[derive(Debug)]
pub enum Notification<'a> {
    Install(elan_dist::Notification<'a>),
    Utils(elan_utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    SetDefaultToolchain(&'a str),
    SetOverrideToolchain(&'a Path, &'a ToolchainDesc),
    LookingForToolchain(&'a ToolchainDesc),
    ToolchainDirectory(&'a Path, &'a ToolchainDesc),
    UpdatingToolchain(&'a ToolchainDesc),
    InstallingToolchain(&'a ToolchainDesc),
    InstalledToolchain(&'a ToolchainDesc),
    UsingExistingToolchain(&'a ToolchainDesc),
    UsingExistingRelease(&'a ToolchainDesc),
    UninstallingToolchain(&'a ToolchainDesc),
    UninstallingObsoleteToolchain(&'a Path),
    UninstalledToolchain(&'a ToolchainDesc),
    ToolchainNotInstalled(&'a ToolchainDesc),
    UpdateHashMatches,
    UpgradingMetadata(&'a str, &'a str),
    MetadataUpgradeNotNeeded(&'a str),
    WritingMetadataVersion(&'a str),
    ReadMetadataVersion(&'a str),
    NonFatalError(&'a Error),
    UpgradeRemovesToolchains,
    MissingFileDuringSelfUninstall(PathBuf),
    SetTelemetry(&'a str),

    TelemetryCleanupError(&'a Error),
}

impl<'a> From<elan_dist::Notification<'a>> for Notification<'a> {
    fn from(n: elan_dist::Notification<'a>) -> Notification<'a> {
        Notification::Install(n)
    }
}
impl<'a> From<elan_utils::Notification<'a>> for Notification<'a> {
    fn from(n: elan_utils::Notification<'a>) -> Notification<'a> {
        Notification::Utils(n)
    }
}
impl<'a> From<temp::Notification<'a>> for Notification<'a> {
    fn from(n: temp::Notification<'a>) -> Notification<'a> {
        Notification::Temp(n)
    }
}

impl Notification<'_> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Install(ref n) => n.level(),
            Utils(ref n) => n.level(),
            Temp(ref n) => n.level(),
            ToolchainDirectory(_, _)
            | LookingForToolchain(_)
            | WritingMetadataVersion(_)
            | InstallingToolchain(_)
            | UpdatingToolchain(_)
            | ReadMetadataVersion(_)
            | InstalledToolchain(_)
            | UpdateHashMatches
            | TelemetryCleanupError(_) => NotificationLevel::Verbose,
            SetDefaultToolchain(_)
            | SetOverrideToolchain(_, _)
            | UsingExistingToolchain(_)
            | UninstallingToolchain(_)
            | UninstallingObsoleteToolchain(_)
            | UninstalledToolchain(_)
            | ToolchainNotInstalled(_)
            | UpgradingMetadata(_, _)
            | MetadataUpgradeNotNeeded(_)
            | SetTelemetry(_) => NotificationLevel::Info,
            NonFatalError(_) => NotificationLevel::Error,
            UpgradeRemovesToolchains
            | MissingFileDuringSelfUninstall(_)
            | UsingExistingRelease(_) => NotificationLevel::Warn,
        }
    }
}

impl Display for Notification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            Install(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            Temp(ref n) => n.fmt(f),
            SetDefaultToolchain(name) => write!(f, "default toolchain set to '{}'", name),
            SetOverrideToolchain(path, name) => {
                write!(
                    f,
                    "override toolchain for '{}' set to '{}'",
                    path.display(),
                    name
                )
            }
            LookingForToolchain(name) => write!(f, "looking for installed toolchain '{}'", name),
            ToolchainDirectory(path, _) => write!(f, "toolchain directory: '{}'", path.display()),
            UpdatingToolchain(name) => write!(f, "updating existing install for '{}'", name),
            InstallingToolchain(name) => write!(f, "installing toolchain '{}'", name),
            InstalledToolchain(name) => write!(f, "toolchain '{}' installed", name),
            UsingExistingToolchain(name) => write!(f, "using existing install for '{}'", name),
            UninstallingToolchain(name) => write!(f, "uninstalling toolchain '{}'", name),
            UninstallingObsoleteToolchain(name) => write!(
                f,
                "uninstalling toolchain '{}' using obsolete format",
                name.display()
            ),
            UninstalledToolchain(name) => write!(f, "toolchain '{}' uninstalled", name),
            ToolchainNotInstalled(name) => write!(f, "no toolchain installed for '{}'", name),
            UpdateHashMatches => {
                write!(f, "toolchain is already up to date")
            }
            UpgradingMetadata(from_ver, to_ver) => {
                write!(
                    f,
                    "upgrading metadata version from '{}' to '{}'",
                    from_ver, to_ver
                )
            }
            MetadataUpgradeNotNeeded(ver) => {
                write!(
                    f,
                    "nothing to upgrade: metadata version is already '{}'",
                    ver
                )
            }
            WritingMetadataVersion(ver) => write!(f, "writing metadata version: '{}'", ver),
            ReadMetadataVersion(ver) => write!(f, "read metadata version: '{}'", ver),
            NonFatalError(e) => write!(f, "{}", e),
            UpgradeRemovesToolchains => write!(
                f,
                "this upgrade will remove all existing toolchains. you will need to reinstall them"
            ),
            MissingFileDuringSelfUninstall(ref p) => {
                write!(
                    f,
                    "expected file does not exist to uninstall: {}",
                    p.display()
                )
            }
            SetTelemetry(telemetry_status) => write!(f, "telemetry set to '{}'", telemetry_status),
            TelemetryCleanupError(e) => write!(f, "unable to remove old telemetry files: '{}'", e),
            UsingExistingRelease(tc) => write!(
                f,
                "failed to query latest release, using existing version '{}'",
                tc
            ),
        }
    }
}
