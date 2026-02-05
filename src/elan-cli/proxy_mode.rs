use crate::common::set_globals;
use crate::errors::*;
use crate::job;
use elan::command::run_command_for_dir;
use elan::{lookup_toolchain_desc, Cfg, OverrideReason};
use elan_utils::utils;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

pub fn main() -> Result<()> {
    crate::self_update::cleanup_self_updater()?;

    let _setup = job::setup();

    let mut args = env::args();

    let arg0 = args.next().map(PathBuf::from);
    let arg0 = arg0
        .as_ref()
        .and_then(|a| a.file_name())
        .and_then(|a| a.to_str());
    let arg0 = arg0.ok_or(ErrorKind::NoExeName)?;

    // Check for a toolchain specifier.
    let arg1 = args.next();
    let toolchain = arg1.as_ref().and_then(|arg1| {
        if arg1.starts_with('+') {
            Some(&arg1[1..])
        } else {
            None
        }
    });

    // Build command args now while we know whether or not to skip arg 1.
    let cmd_args: Vec<_> = if toolchain.is_none() {
        env::args_os().skip(1).collect()
    } else {
        env::args_os().skip(2).collect()
    };

    let cfg = set_globals(false)?;
    direct_proxy(&cfg, arg0, toolchain, &cmd_args)?;

    Ok(())
}

fn direct_proxy(cfg: &Cfg, arg0: &str, toolchain: Option<&str>, args: &[OsString]) -> Result<()> {
    let cmd = match toolchain {
        None => {
            let cwd = utils::current_dir()?;
            let (toolchain, reason) = cfg.toolchain_for_dir(&cwd)?;

            // Print a notice when using a directory override set via `elan override set`,
            // unless suppressed via environment variable
            if let Some(OverrideReason::OverrideDB(ref path)) = reason {
                if env::var_os("ELAN_NO_OVERRIDE_NOTICE").is_none() {
                    note!(
                        "using toolchain '{}' from override set on '{}'",
                        toolchain.name(),
                        path.display()
                    );
                    note!(
                        "to remove: elan override unset --path '{}' | to suppress: ELAN_NO_OVERRIDE_NOTICE=1",
                        path.display()
                    );
                }
            }

            toolchain.create_command(arg0)?
        }
        Some(tc) => {
            let desc = lookup_toolchain_desc(cfg, tc)?;
            cfg.create_command_for_toolchain(&desc, true, arg0)?
        }
    };
    Ok(run_command_for_dir(cmd, arg0, args)?)
}
