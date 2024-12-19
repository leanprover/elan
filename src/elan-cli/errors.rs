#![allow(dead_code)]

use std::io;
use std::path::PathBuf;

use elan_dist::{self, temp};
use error_chain::error_chain;

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
