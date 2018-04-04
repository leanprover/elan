extern crate gcc;

use std::env;
use gcc::windows_registry::{self, VsVers};

fn main() {
    println!("leanpkg:rustc-link-lib=dylib=msi");
    println!("leanpkg:rustc-link-lib=dylib=user32");
    println!("leanpkg:rustc-link-lib=dylib=mincore");

    // Part of WIX SDK
    println!("leanpkg:rustc-link-lib=static=wcautil");
    println!("leanpkg:rustc-link-lib=static=dutil");

    let wix_path = env::var("WIX").expect("WIX must be installed, and 'WIX' environment variable must be set");

    // For the correct WIX library path, we need to know which VS version we are using.
    // We use the `gcc` crate's functionality to do this, which should always match what rustc is doing.
    let vs_version = windows_registry::find_vs_version().expect("Cannot find VS version");
    let vs_version_string = match vs_version {
        VsVers::Vs14 => "VS2015",
        VsVers::Vs15 => "VS2017",
        VsVers::Vs12 => panic!("Unsupported VS version: Vs12"),
        _ => panic!("Unsupported VS version") // FIXME: should use {:?}, but `VsVers` does not yet implement `Debug`
    };

    println!("leanpkg:warning=Using WIX libraries for VS version: {}", vs_version_string);

    let target_arch = env::var("LEANPKG_CFG_TARGET_ARCH").expect("cannot read LEANPKG_CFG_TARGET_ARCH in build script");
    let target_arch = match target_arch.as_str() {
        "x86" => "x86",
        "x86_64" => "x64",
        other => panic!("Target architecture {} not supported by WIX.", other)
    };
    
    // Tell leanpkg about the WIX SDK path for `wcautil.lib` and `dutil.lib`
    println!("leanpkg:rustc-link-search=native={}SDK\\{}\\lib\\{}", wix_path, vs_version_string, target_arch);
}
