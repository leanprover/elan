use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target = env::var("RELEASE_TARGET_NAME").or(env::var("TARGET")).unwrap();

    File::create(out_dir.join("target.txt")).unwrap().write_all(target.as_bytes()).unwrap();
    println!("leanpkg:rerun-if-changed=build.rs");
}
