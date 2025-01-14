use crate::common;
use crate::errors::*;
use crate::self_update::{self, InstallOpts};
use clap::{App, AppSettings, Arg};
use std::env;

pub fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    let arg1 = args.get(1).map(|a| &**a);

    // Secret command used during self-update. Not for users.
    if arg1 == Some("--self-replace") {
        return self_update::self_replace();
    }
    // XXX: If you change anything here, please make the same changes in elan-init.sh
    let cli = App::new("elan-init")
        .version(common::version())
        .about("The installer for elan")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enable verbose output"),
        )
        .arg(
            Arg::with_name("no-prompt")
                .short("y")
                .help("Disable confirmation prompt."),
        )
        .arg(
            Arg::with_name("default-toolchain")
                .long("default-toolchain")
                .takes_value(true)
                .help("Choose a default toolchain"),
        )
        .arg(
            Arg::with_name("no-modify-path")
                .long("no-modify-path")
                .help("Don't configure the PATH environment variable"),
        );

    let matches = cli.get_matches();
    let no_prompt = matches.is_present("no-prompt");
    let verbose = matches.is_present("verbose");
    let default_toolchain = matches.value_of("default-toolchain").unwrap_or("stable");
    let no_modify_path = matches.is_present("no-modify-path");

    let opts = InstallOpts {
        default_toolchain: default_toolchain.to_owned(),
        no_modify_path,
    };

    self_update::install(no_prompt, verbose, opts)?;

    Ok(())
}
