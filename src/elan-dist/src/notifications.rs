use crate::errors::*;
use crate::manifest::Component;
use crate::temp;
use elan_utils;
use elan_utils::notify::NotificationLevel;
use std::fmt::{self, Display};
use std::path::Path;

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(elan_utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    Extracting(&'a Path, &'a Path),
    ComponentAlreadyInstalled(&'a Component),
    CantReadUpdateHash(&'a Path),
    NoUpdateHash(&'a Path),
    ChecksumValid(&'a str),
    SignatureValid(&'a str),
    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
    RollingBack,
    ExtensionNotInstalled(&'a Component),
    NonFatalError(&'a Error),
    MissingInstalledComponent(&'a str),
    DownloadingComponent(&'a str),
    InstallingComponent(&'a str),
    RemovingComponent(&'a str),
    DownloadingManifest(&'a str),
    DownloadedManifest(&'a str, Option<&'a str>),
    DownloadingLegacyManifest,
    ManifestChecksumFailedHack,
    NewVersionAvailable(String),
    WaitingForFileLock(&'a Path, &'a str),
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

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.level(),
            Utils(ref n) => n.level(),
            ChecksumValid(_)
            | NoUpdateHash(_)
            | FileAlreadyDownloaded
            | DownloadingLegacyManifest => NotificationLevel::Verbose,
            Extracting(_, _)
            | SignatureValid(_)
            | DownloadingComponent(_)
            | InstallingComponent(_)
            | RemovingComponent(_)
            | ComponentAlreadyInstalled(_)
            | ManifestChecksumFailedHack
            | RollingBack
            | DownloadingManifest(_)
            | NewVersionAvailable(_)
            | WaitingForFileLock(_, _)
            | DownloadedManifest(_, _) => NotificationLevel::Info,
            CantReadUpdateHash(_)
            | ExtensionNotInstalled(_)
            | MissingInstalledComponent(_)
            | CachedFileChecksumFailed => NotificationLevel::Warn,
            NonFatalError(_) => NotificationLevel::Error,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            Extracting(_, _) => write!(f, "extracting..."),
            ComponentAlreadyInstalled(ref c) => {
                write!(f, "component {} is up to date", c.description())
            }
            CantReadUpdateHash(path) => {
                write!(
                    f,
                    "can't read update hash file: '{}', can't skip update...",
                    path.display()
                )
            }
            NoUpdateHash(path) => write!(f, "no update hash at: '{}'", path.display()),
            ChecksumValid(_) => write!(f, "checksum passed"),
            SignatureValid(_) => write!(f, "signature valid"),
            FileAlreadyDownloaded => write!(f, "reusing previously downloaded file"),
            CachedFileChecksumFailed => write!(f, "bad checksum for cached download"),
            RollingBack => write!(f, "rolling back changes"),
            ExtensionNotInstalled(c) => {
                write!(f, "extension '{}' was not installed", c.name())
            }
            NonFatalError(e) => write!(f, "{}", e),
            MissingInstalledComponent(c) => {
                write!(f, "during uninstall component {} was not found", c)
            }
            DownloadingComponent(c) => write!(f, "downloading {}", c),
            InstallingComponent(c) => write!(f, "installing {}", c),
            RemovingComponent(c) => write!(f, "removing {}", c),
            DownloadingManifest(t) => write!(f, "syncing channel updates for '{}'", t),
            DownloadedManifest(date, Some(version)) => {
                write!(f, "latest update on {}, lean version {}", date, version)
            }
            DownloadedManifest(date, None) => {
                write!(f, "latest update on {}, no lean version", date)
            }
            DownloadingLegacyManifest => write!(f, "manifest not found. trying legacy manifest"),
            ManifestChecksumFailedHack => {
                write!(f, "update not yet available, sorry! try again later")
            }
            NewVersionAvailable(ref version) => {
                write!(
                    f,
                    "Version {version} of elan is available! Use `elan self update` to update."
                )
            }
            WaitingForFileLock(path, pid) => {
                write!(
                    f,
                    "waiting for previous installation request to finish ({}, held by PID {})",
                    path.display(),
                    pid
                )
            }
        }
    }
}
