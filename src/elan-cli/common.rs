//! Just a dumping ground for cli stuff

use crate::errors::*;
use crate::term2;
use elan::{Cfg, Notification, Toolchain};
use elan_dist::dist::ToolchainDesc;
use elan_utils::notify::NotificationLevel;
use elan_utils::utils;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use wait_timeout::ChildExt;

pub fn confirm(question: &str, default: bool) -> Result<bool> {
    print!("{} ", question);
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input {
        "y" | "Y" => true,
        "n" | "N" => false,
        "" => default,
        _ => false,
    };

    println!();

    Ok(r)
}

pub enum Confirm {
    Yes,
    No,
    Advanced,
}

pub fn confirm_advanced() -> Result<Confirm> {
    println!();
    println!("1) Proceed with installation (default)");
    println!("2) Customize installation");
    println!("3) Cancel installation");

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input {
        "1" | "" => Confirm::Yes,
        "2" => Confirm::Advanced,
        _ => Confirm::No,
    };

    println!();

    Ok(r)
}

pub fn question_str(question: &str, default: &str) -> Result<String> {
    println!("{}", question);
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    println!();

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

pub fn question_bool(question: &str, default: bool) -> Result<bool> {
    println!("{}", question);

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    println!();

    if input.is_empty() {
        Ok(default)
    } else {
        match &*input {
            "y" | "Y" | "yes" => Ok(true),
            "n" | "N" | "no" => Ok(false),
            _ => Ok(default),
        }
    }
}

pub fn read_line() -> Result<String> {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    lines
        .next()
        .and_then(|l| l.ok())
        .ok_or("unable to read from stdin for confirmation".into())
}

pub fn set_globals(verbose: bool) -> Result<Cfg> {
    use crate::download_tracker::DownloadTracker;
    use std::cell::RefCell;

    let download_tracker = RefCell::new(DownloadTracker::new());

    Ok(Cfg::from_env(Arc::new(move |n: Notification<'_>| {
        if download_tracker.borrow_mut().handle_notification(&n) {
            return;
        }

        match n.level() {
            NotificationLevel::Verbose => {
                if verbose {
                    verbose!("{}", n);
                }
            }
            NotificationLevel::Info => {
                info!("{}", n);
            }
            NotificationLevel::Warn => {
                warn!("{}", n);
            }
            NotificationLevel::Error => {
                err!("{}", n);
            }
        }
    }))?)
}

pub fn show_channel_update(cfg: &Cfg, desc: &ToolchainDesc) -> Result<()> {
    let toolchain = &cfg.get_toolchain(desc, false).expect("");
    let version = lean_version(toolchain);
    let name = desc.to_string();

    let banner = "installed";
    let color = Some(term2::color::BRIGHT_GREEN);

    let mut t = term2::stdout();

    let _ = t.attr(term2::Attr::Bold);
    if let Some(color) = color {
        let _ = t.fg(color);
    }
    let _ = write!(t, "{} ", name);
    let _ = write!(t, "{}", banner);
    let _ = t.reset();
    let _ = writeln!(t, " - {}", version);
    let _ = writeln!(t);

    Ok(())
}

pub fn lean_version(toolchain: &Toolchain<'_>) -> String {
    if toolchain.exists() {
        let lean_path = toolchain.binary_file("lean");
        if utils::is_file(&lean_path) {
            let mut cmd = Command::new(&lean_path);
            cmd.arg("--version");
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            // some toolchains are faulty with some combinations of platforms and
            // may fail to launch but also to timely terminate.
            // (known cases include Lean 1.3.0 through 1.10.0 in recent macOS Sierra.)
            // we guard against such cases by enforcing a reasonable timeout to read.
            let mut line1 = None;
            if let Ok(mut child) = cmd.spawn() {
                let timeout = Duration::new(10, 0);
                match child.wait_timeout(timeout) {
                    Ok(Some(status)) if status.success() => {
                        let out = child
                            .stdout
                            .expect("Child::stdout requested but not present");
                        let mut line = String::new();
                        if BufReader::new(out).read_line(&mut line).is_ok() {
                            let lineend = line.trim_end_matches(&['\r', '\n'][..]).len();
                            line.truncate(lineend);
                            line1 = Some(line);
                        }
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        return String::from("(timeout reading lean version)");
                    }
                    Ok(Some(_)) | Err(_) => {}
                }
            }

            if let Some(line1) = line1 {
                line1.to_owned()
            } else {
                String::from("(error reading lean version)")
            }
        } else {
            String::from("(lean does not exist)")
        }
    } else {
        String::from("(toolchain will be installed on first use)")
    }
}

pub fn list_overrides(cfg: &Cfg) -> Result<()> {
    let overrides = cfg.settings_file.with(|s| Ok(s.overrides.clone()))?;

    if overrides.is_empty() {
        println!("no overrides");
    } else {
        let mut any_not_exist = false;
        for (k, v) in overrides {
            let dir_exists = Path::new(&k).is_dir();
            if !dir_exists {
                any_not_exist = true;
            }
            println!(
                "{:<40}\t{:<20}",
                utils::format_path_for_display(&k)
                    + if dir_exists { "" } else { " (not a directory)" },
                v
            )
        }
        if any_not_exist {
            println!();
            info!(
                "you may remove overrides for non-existent directories with
`elan override unset --nonexistent`"
            );
        }
    }
    Ok(())
}

pub fn version() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))
    )
}

pub fn report_error(e: &Error) {
    err!("{}", e);

    for e in e.iter().skip(1) {
        info!("caused by: {}", e);
    }

    if show_backtrace() {
        if let Some(backtrace) = e.backtrace() {
            info!("backtrace:");
            println!();
            println!("{:?}", backtrace);
        }
    }

    fn show_backtrace() -> bool {
        use std::env;
        use std::ops::Deref;

        if env::var("RUST_BACKTRACE").as_ref().map(Deref::deref) == Ok("1") {
            return true;
        }

        for arg in env::args() {
            if arg == "-v" || arg == "--verbose" {
                return true;
            }
        }

        false
    }
}
