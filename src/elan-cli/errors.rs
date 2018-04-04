#![allow(dead_code)]

use std::io;
use std::path::PathBuf;

use elan;
use elan_dist::{self, temp};
use elan_utils;

error_chain! {
    links {
        Elan(elan::Error, elan::ErrorKind);
        Dist(elan_dist::Error, elan_dist::ErrorKind);
        Utils(elan_utils::Error, elan_utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
        Io(io::Error);
    }

    errors {
        PermissionDenied {
            description("permission denied")
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        InfiniteRecursion {
            description("infinite recursion detected")
        }
        NoExeName {
            description("couldn't determine self executable name")
        }
        NotSelfInstalled(p: PathBuf) {
            description("elan is not installed")
            display("elan is not installed at '{}'", p.display())
        }
        WindowsUninstallMadness {
            description("failure during windows uninstall")
        }
    }
}
