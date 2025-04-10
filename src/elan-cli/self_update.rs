//! Self-installation and updating
//!
//! This is the installer at the heart of Lean. If it breaks
//! everything breaks. It is conceptually very simple, as elan is
//! distributed as a single binary, and installation mostly requires
//! copying it into place. There are some tricky bits though, mostly
//! because of workarounds to self-delete an exe on Windows.
//!
//! During install (as `elan-init`):
//!
//! * copy the self exe to $ELAN_HOME/bin
//! * hardlink lean, etc to *that*
//! * update the PATH in a system-specific way
//! * run the equivalent of `elan default stable`
//!
//! During upgrade (`elan self upgrade`):
//!
//! * download elan-init to $ELAN_HOME/bin/elan-init
//! * run elan-init with appropriate flags to indicate
//!   this is a self-upgrade
//! * elan-init copies bins and hardlinks into place. On windows
//!   this happens *after* the upgrade command exits successfully.
//!
//! During uninstall (`elan self uninstall`):
//!
//! * Delete `$ELAN_HOME`.
//! * Delete everything in `$ELAN_HOME`, including
//!   the elan binary and its hardlinks
//!
//! Deleting the running binary during uninstall is tricky
//! and racy on Windows.

use crate::common::{self, Confirm};
use crate::errors::*;
use crate::term2;
use elan::install;
use elan::lookup_toolchain_desc;
use elan::lookup_unresolved_toolchain_desc;
use elan::Notification;
use elan::Toolchain;
use elan_dist::dist;
use elan_dist::dist::ToolchainDesc;
use elan_utils::utils;
use regex::Regex;
use same_file::Handle;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{self, Command};
use tempfile::tempdir;

pub struct InstallOpts {
    pub default_toolchain: String,
    pub no_modify_path: bool,
}

// The big installation messages. These are macros because the first
// argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg: expr) => {
        concat!(
            r"
# Welcome to Lean!

This will download and install Elan, a tool for managing different Lean versions used in
packages you create or download. It will also install a default version of Lean and its package
manager, lake, for editing files not belonging to any package.

It will add the `lake`, `lean`, and `elan` commands to
Elan's bin directory, located at:

    {elan_home_bin}

",
            $platform_msg,
            r#"

You can uninstall at any time with `elan self uninstall` and
these changes will be reverted.
"#
        )
    };
}

macro_rules! pre_install_msg_unix {
    () => {
        pre_install_msg_template!(
            "This path will then be added to your `PATH` environment variable by
modifying the profile file{plural} located at:

{rcfiles}"
        )
    };
}

macro_rules! pre_install_msg_win {
    () => {
        pre_install_msg_template!(
            "This path will then be added to your `PATH` environment variable by
modifying the `HKEY_CURRENT_USER/Environment/PATH` registry key."
        )
    };
}

macro_rules! pre_install_msg_no_modify_path {
    () => {
        pre_install_msg_template!(
            "This path needs to be in your `PATH` environment variable,
but will not be added automatically."
        )
    };
}

macro_rules! post_install_msg_unix {
    () => {
        r"# Elan is installed now. Great!

To get started you need Elan's bin directory ({elan_home}/bin) in your `PATH`
environment variable. Next time you log in this will be done
automatically.

To configure your current shell run `source {elan_home}/env`
"
    };
}

macro_rules! post_install_msg_win {
    () => {
        r"# Elan is installed now. Great!

To get started you need Elan's bin directory ({elan_home}\bin) in your `PATH`
environment variable. Future applications will automatically have the
correct environment, but you may need to restart your current shell.
"
    };
}

macro_rules! post_install_msg_unix_no_modify_path {
    () => {
        r"# Elan is installed now. Great!

To get started you need Elan's bin directory ({elan_home}/bin) in your `PATH`
environment variable.

To configure your current shell run `source {elan_home}/env`
"
    };
}

macro_rules! post_install_msg_win_no_modify_path {
    () => {
        r"# Elan is installed now. Great!

To get started you need Elan's bin directory ({elan_home}\bin) in your `PATH`
environment variable. This has not been done automatically.
"
    };
}

macro_rules! pre_uninstall_msg {
    () => {
        r"This will uninstall all Lean toolchains and data, and remove
`{elan_home}/bin` from your `PATH` environment variable.

"
    };
}

static TOOLS: &[&str] = &[
    "lean",
    "leanpkg",
    "leanchecker",
    "leanc",
    "leanmake",
    "lake",
];

static UPDATE_ROOT: &str = "https://github.com/leanprover/elan/releases/download";

/// `ELAN_HOME` suitable for display, possibly with $HOME
/// substituted for the directory prefix
fn canonical_elan_home() -> Result<String> {
    let path = utils::elan_home()?;
    let mut path_str = path.to_string_lossy().to_string();

    let default_elan_home = utils::home_dir()
        .unwrap_or(PathBuf::from("."))
        .join(".elan");
    if default_elan_home == path {
        if cfg!(unix) {
            path_str = String::from("$HOME/.elan");
        } else {
            path_str = String::from(r"%USERPROFILE%\.elan");
        }
    }

    Ok(path_str)
}

fn clean_up_old_state() -> Result<()> {
    let cfg = &(common::set_globals(false)?);
    for tc in cfg.list_toolchains()? {
        let res = lookup_unresolved_toolchain_desc(cfg, &tc.to_string());
        if let Ok(desc) = res {
            if desc.0 == tc
                && !matches!(
                    desc.0,
                    ToolchainDesc::Remote {
                        from_channel: Some(_),
                        ..
                    }
                )
            {
                continue;
            }
        }
        let t = Toolchain::from(cfg, &tc);
        (cfg.notify_handler)(Notification::UninstallingObsoleteToolchain(t.path()));
        install::uninstall(t.path(), &|n| (cfg.notify_handler)(n.into()))?;
    }
    Ok(())
}

/// Installing is a simple matter of coping the running binary to
/// `ELAN_HOME`/bin, hardlinking the various Lean tools to it,
/// and adding `ELAN_HOME`/bin to PATH.
pub fn install(no_prompt: bool, verbose: bool, mut opts: InstallOpts) -> Result<()> {
    check_existence_of_lean_in_path(no_prompt)?;
    do_anti_sudo_check(no_prompt)?;

    if !no_prompt {
        let msg = &(pre_install_msg(opts.no_modify_path)?);

        term2::stdout().md(msg);

        loop {
            term2::stdout().md(current_install_opts(&opts));
            match common::confirm_advanced()? {
                Confirm::No => {
                    info!("aborting installation");
                    return Ok(());
                }
                Confirm::Yes => {
                    break;
                }
                Confirm::Advanced => {
                    opts = customize_install(opts)?;
                }
            }
        }
    }

    let install_res: Result<()> = (|| {
        install_bins()?;
        if !opts.no_modify_path {
            do_add_to_path(&get_add_path_methods())?;
        }
        if opts.default_toolchain != "none" {
            let cfg = &(common::set_globals(verbose)?);
            // sanity-check reference
            let _ = lookup_toolchain_desc(cfg, &opts.default_toolchain)?;
            cfg.set_default(&opts.default_toolchain)?;
        }

        if cfg!(unix) {
            let env_file = &utils::elan_home()?.join("env");
            let env_str = &format!("{}\n", shell_export_string()?);
            utils::write_file("env", env_file, env_str)?;
        }

        clean_up_old_state()
    })();

    if let Err(ref e) = install_res {
        common::report_error(e);

        process::exit(1);
    }

    // More helpful advice, skip if -y
    if !no_prompt {
        let elan_home = canonical_elan_home()?;
        let msg = if !opts.no_modify_path {
            if cfg!(unix) {
                format!(post_install_msg_unix!(), elan_home = elan_home)
            } else {
                format!(post_install_msg_win!(), elan_home = elan_home)
            }
        } else if cfg!(unix) {
            format!(
                post_install_msg_unix_no_modify_path!(),
                elan_home = elan_home
            )
        } else {
            format!(
                post_install_msg_win_no_modify_path!(),
                elan_home = elan_home
            )
        };
        term2::stdout().md(msg);
    }

    Ok(())
}

fn lean_exists_in_path() -> Result<()> {
    // Ignore lean if present in $HOME/.elan/bin
    fn ignore_paths(path: &PathBuf) -> bool {
        !path
            .components()
            .any(|c| c == Component::Normal(".elan".as_ref()))
    }

    if let Some(paths) = env::var_os("PATH") {
        let paths = env::split_paths(&paths).filter(ignore_paths);

        for path in paths {
            let lean = path.join(format!("lean{}", EXE_SUFFIX));

            if lean.exists() {
                return Err(path.to_str().unwrap().into());
            }
        }
    }
    Ok(())
}

fn check_existence_of_lean_in_path(no_prompt: bool) -> Result<()> {
    // Only the test runner should set this
    let skip_check = env::var_os("ELAN_INIT_SKIP_PATH_CHECK");

    // Ignore this check if called with no prompt (-y) or if the environment variable is set
    if no_prompt || skip_check == Some("yes".into()) {
        return Ok(());
    }

    if let Err(path) = lean_exists_in_path() {
        err!("it looks like you have an existing installation of Lean at:");
        err!("{}", path);
        err!("elan cannot be installed alongside Lean. Please uninstall first");
        err!("if this is what you want, restart the installation with `-y'");
        Err("cannot install while Lean is installed".into())
    } else {
        Ok(())
    }
}

// If the user is trying to install with sudo, on some systems this will
// result in writing root-owned files to the user's home directory, because
// sudo is configured not to change $HOME. Don't let that bogosity happen.
#[allow(dead_code)]
fn do_anti_sudo_check(no_prompt: bool) -> Result<()> {
    #[cfg(unix)]
    pub fn home_mismatch() -> bool {
        use libc as c;

        use std::ffi::CStr;
        use std::mem::MaybeUninit;
        use std::ops::Deref;
        use std::ptr;

        // test runner should set this, nothing else
        if env::var("ELAN_INIT_SKIP_SUDO_CHECK")
            .as_ref()
            .map(Deref::deref)
            .ok()
            == Some("yes")
        {
            return false;
        }
        let mut buf = [0 as c::c_char; 1024];
        let mut pwd = MaybeUninit::<c::passwd>::uninit();
        let mut pwdp: *mut c::passwd = ptr::null_mut();
        let rv = unsafe {
            c::getpwuid_r(
                c::geteuid(),
                pwd.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut pwdp,
            )
        };
        if rv != 0 {
            warn!("getpwuid_r: couldn't get user data ({})", rv);
            return false;
        }
        if pwdp.is_null() {
            warn!("getpwuid_r: couldn't get user data");
            return false;
        }
        let pw_dir = unsafe { CStr::from_ptr(pwd.assume_init().pw_dir) }
            .to_str()
            .ok();
        let env_home = env::var_os("HOME");
        let env_home = env_home.as_deref();
        match (env_home, pw_dir) {
            (None, _) | (_, None) => false,
            (Some(eh), Some(pd)) => eh != pd,
        }
    }

    #[cfg(not(unix))]
    pub fn home_mismatch() -> bool {
        false
    }

    match (home_mismatch(), no_prompt) {
        (false, _) => (),
        (true, false) => {
            err!("$HOME differs from euid-obtained home directory: you may be using sudo");
            err!("if this is what you want, restart the installation with `-y'");
            process::exit(1);
        }
        (true, true) => {
            warn!("$HOME differs from euid-obtained home directory: you may be using sudo");
        }
    }

    Ok(())
}

fn pre_install_msg(no_modify_path: bool) -> Result<String> {
    let elan_home = utils::elan_home()?;
    let elan_home_bin = elan_home.join("bin");

    if !no_modify_path {
        if cfg!(unix) {
            let add_path_methods = get_add_path_methods();
            let rcfiles = add_path_methods
                .into_iter()
                .filter_map(|m| {
                    if let PathUpdateMethod::RcFile(path) = m {
                        Some(format!("{}", path.display()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let plural = if rcfiles.len() > 1 { "s" } else { "" };
            let rcfiles = rcfiles
                .into_iter()
                .map(|f| format!("    {}", f))
                .collect::<Vec<_>>();
            let rcfiles = rcfiles.join("\n");
            Ok(format!(
                pre_install_msg_unix!(),
                elan_home_bin = elan_home_bin.display(),
                plural = plural,
                rcfiles = rcfiles
            ))
        } else {
            Ok(format!(
                pre_install_msg_win!(),
                elan_home_bin = elan_home_bin.display()
            ))
        }
    } else {
        Ok(format!(
            pre_install_msg_no_modify_path!(),
            elan_home_bin = elan_home_bin.display()
        ))
    }
}

fn current_install_opts(opts: &InstallOpts) -> String {
    format!(
        r"Current installation options:

- `   `default toolchain: `{}`
- modify PATH variable: `{}`
",
        opts.default_toolchain,
        if !opts.no_modify_path { "yes" } else { "no" }
    )
}

// Interactive editing of the install options
fn customize_install(mut opts: InstallOpts) -> Result<InstallOpts> {
    println!(
        "I'm going to ask you the value of each these installation options.\n\
         You may simply press the Enter key to leave unchanged."
    );

    println!();

    opts.default_toolchain = common::question_str(
        "Default toolchain? (stable/nightly/<specific version>/none)",
        &opts.default_toolchain,
    )?;

    opts.no_modify_path =
        !common::question_bool("Modify PATH variable? (y/n)", !opts.no_modify_path)?;

    Ok(opts)
}

fn install_bins() -> Result<()> {
    let bin_path = &utils::elan_home()?.join("bin");
    let this_exe_path = &(utils::current_exe()?);
    let elan_path = &bin_path.join(format!("elan{}", EXE_SUFFIX));

    utils::ensure_dir_exists("bin", bin_path, &|_| {})?;
    // NB: Even on Linux we can't just copy the new binary over the (running)
    // old binary; we must unlink it first.
    if elan_path.exists() {
        utils::remove_file("elan-bin", elan_path)?;
    }
    utils::copy_file(this_exe_path, elan_path)?;
    utils::make_executable(elan_path)?;
    install_proxies()
}

pub fn install_proxies() -> Result<()> {
    let bin_path = &utils::elan_home()?.join("bin");
    let elan_path = &bin_path.join(format!("elan{}", EXE_SUFFIX));

    let elan = Handle::from_path(elan_path)?;

    let mut tool_handles = Vec::new();
    let mut link_afterwards = Vec::new();

    // Try to hardlink all the Lean exes to the elan exe. Some systems,
    // like Android, does not support hardlinks, so we fallback to symlinks.
    //
    // Note that this function may not be running in the context of a fresh
    // self update but rather as part of a normal update to fill in missing
    // proxies. In that case our process may actually have the `elan.exe`
    // file open, and on systems like Windows that means that you can't
    // even remove other hard links to the same file. Basically if we have
    // `elan.exe` open and running and `lean.exe` is a hard link to that
    // file, we can't remove `lean.exe`.
    //
    // To avoid unnecessary errors from being returned here we use the
    // `same-file` crate and its `Handle` type to avoid clobbering hard links
    // that are already valid. If a hard link already points to the
    // `elan.exe` file then we leave it alone and move to the next one.
    //
    // As yet one final caveat, when we're looking at handles for files we can't
    // actually delete files (they'll say they're deleted but they won't
    // actually be on Windows). As a result we manually drop all the
    // `tool_handles` later on. This'll allow us, afterwards, to actually
    // overwrite all the previous hard links with new ones.
    for tool in TOOLS {
        let tool_path = bin_path.join(format!("{}{}", tool, EXE_SUFFIX));
        if let Ok(handle) = Handle::from_path(&tool_path) {
            tool_handles.push(handle);
            if elan == *tool_handles.last().unwrap() {
                continue;
            }
        }
        link_afterwards.push(tool_path);
    }

    drop(tool_handles);
    for path in link_afterwards {
        utils::hard_or_symlink_file(elan_path, &path)?;
    }

    Ok(())
}

pub fn uninstall(no_prompt: bool) -> Result<()> {
    if elan::install::NEVER_SELF_UPDATE {
        err!("self-uninstall is disabled for this build of elan");
        err!("you should probably use your system package manager to uninstall elan");
        process::exit(1);
    }

    if cfg!(feature = "msi-installed") {
        // Get the product code of the MSI installer from the registry
        // and spawn `msiexec /x`, then exit immediately
        let product_code = get_msi_product_code()?;
        Command::new("msiexec")
            .arg("/x")
            .arg(product_code)
            .spawn()
            .chain_err(|| ErrorKind::WindowsUninstallMadness)?;
        process::exit(0);
    }

    let elan_home = &(utils::elan_home()?);

    if !elan_home.join(format!("bin/elan{}", EXE_SUFFIX)).exists() {
        return Err(ErrorKind::NotSelfInstalled(elan_home.clone()).into());
    }

    if !no_prompt {
        println!();
        let msg = &format!(pre_uninstall_msg!(), elan_home = canonical_elan_home()?);
        term2::stdout().md(msg);
        if !common::confirm("\nContinue? (y/N)", false)? {
            info!("aborting uninstallation");
            return Ok(());
        }
    }

    let read_dir_err = "failure reading directory";

    info!("removing elan home");

    // Remove ELAN_HOME/bin from PATH
    let remove_path_methods = &(get_remove_path_methods()?);
    do_remove_from_path(remove_path_methods)?;

    // Delete everything in ELAN_HOME *except* the elan bin

    // First everything except the bin directory
    for dirent in fs::read_dir(elan_home).chain_err(|| read_dir_err)? {
        let dirent = dirent.chain_err(|| read_dir_err)?;
        if dirent.file_name().to_str() != Some("bin") {
            if dirent.path().is_dir() {
                utils::remove_dir("elan_home", &dirent.path(), &|_| {})?;
            } else {
                utils::remove_file("elan_home", &dirent.path())?;
            }
        }
    }

    // Then everything in bin except elan and tools. These can't be unlinked
    // until this process exits (on windows).
    let tools = TOOLS.iter().map(|t| format!("{}{}", t, EXE_SUFFIX));
    let tools: Vec<_> = tools.chain(vec![format!("elan{}", EXE_SUFFIX)]).collect();
    for dirent in fs::read_dir(elan_home.join("bin")).chain_err(|| read_dir_err)? {
        let dirent = dirent.chain_err(|| read_dir_err)?;
        let name = dirent.file_name();
        let file_is_tool = name.to_str().map(|n| tools.iter().any(|t| *t == n));
        if file_is_tool == Some(false) {
            if dirent.path().is_dir() {
                utils::remove_dir("elan_home", &dirent.path(), &|_| {})?;
            } else {
                utils::remove_file("elan_home", &dirent.path())?;
            }
        }
    }

    info!("removing elan binaries");

    // Delete elan. This is tricky because this is *probably*
    // the running executable and on Windows can't be unlinked until
    // the process exits.
    delete_elan_and_elan_home()?;

    info!("elan is uninstalled");

    process::exit(0);
}

#[cfg(not(feature = "msi-installed"))]
fn get_msi_product_code() -> Result<String> {
    unreachable!()
}

#[cfg(feature = "msi-installed")]
fn get_msi_product_code() -> Result<String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root.open_subkey_with_flags("SOFTWARE\\elan", KEY_READ);

    match environment {
        Ok(env) => match env.get_value("InstalledProductCode") {
            Ok(val) => Ok(val),
            Err(e) => Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness),
        },
        Err(e) => Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness),
    }
}

#[cfg(unix)]
fn delete_elan_and_elan_home() -> Result<()> {
    let elan_home = &(utils::elan_home()?);
    utils::remove_dir("elan_home", elan_home, &|_| ())?;

    Ok(())
}

// The last step of uninstallation is to delete *this binary*,
// elan.exe and the ELAN_HOME that contains it. On Unix, this
// works fine. On Windows you can't delete files while they are open,
// like when they are running.
//
// Here's what we're going to do:
// - Copy elan to a temporary file in
//   ELAN_HOME/../elan-gc-$random.exe.
// - Open the gc exe with the FILE_FLAG_DELETE_ON_CLOSE and
//   FILE_SHARE_DELETE flags. This is going to be the last
//   file to remove, and the OS is going to do it for us.
//   This file is opened as inheritable so that subsequent
//   processes created with the option to inherit handles
//   will also keep them open.
// - Run the gc exe, which waits for the original elan
//   process to close, then deletes ELAN_HOME. This process
//   has inherited a FILE_FLAG_DELETE_ON_CLOSE handle to itself.
// - Finally, spawn yet another system binary with the inherit handles
//   flag, so *it* inherits the FILE_FLAG_DELETE_ON_CLOSE handle to
//   the gc exe. If the gc exe exits before the system exe then at
//   last it will be deleted when the handle closes.
//
// This is the DELETE_ON_CLOSE method from
// http://www.catch22.net/tuts/self-deleting-executables
//
// ... which doesn't actually work because Windows won't really
// delete a FILE_FLAG_DELETE_ON_CLOSE process when it exits.
//
// .. augmented with this SO answer
// http://stackoverflow.com/questions/10319526/understanding-a-self-deleting-program-in-c
#[cfg(windows)]
fn delete_elan_and_elan_home() -> Result<()> {
    use std::thread;
    use std::time::Duration;

    // ELAN_HOME, hopefully empty except for bin/elan.exe
    let ref elan_home = utils::elan_home()?;
    // The elan.exe bin
    let ref elan_path = elan_home.join(&format!("bin/elan{}", EXE_SUFFIX));

    // The directory containing ELAN_HOME
    let work_path = elan_home
        .parent()
        .expect("ELAN_HOME doesn't have a parent?");

    // Generate a unique name for the files we're about to move out
    // of ELAN_HOME.
    let numbah: u32 = rand::random();
    let gc_exe = work_path.join(&format!("elan-gc-{:x}.exe", numbah));

    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
    use winapi::um::winbase::FILE_FLAG_DELETE_ON_CLOSE;
    use winapi::um::winnt::{FILE_SHARE_DELETE, FILE_SHARE_READ, GENERIC_READ};

    unsafe {
        // Copy elan (probably this process's exe) to the gc exe
        utils::copy_file(elan_path, &gc_exe)?;

        let mut gc_exe_win: Vec<_> = gc_exe.as_os_str().encode_wide().collect();
        gc_exe_win.push(0);

        // Open an inheritable handle to the gc exe marked
        // FILE_FLAG_DELETE_ON_CLOSE. This will be inherited
        // by subsequent processes.
        let mut sa = mem::zeroed::<SECURITY_ATTRIBUTES>();
        sa.nLength = mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD;
        sa.bInheritHandle = 1;

        let gc_handle = CreateFileW(
            gc_exe_win.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            &mut sa,
            OPEN_EXISTING,
            FILE_FLAG_DELETE_ON_CLOSE,
            ptr::null_mut(),
        );

        if gc_handle == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let _g = scopeguard::guard(gc_handle, |h| {
            let _ = CloseHandle(h);
        });

        Command::new(gc_exe)
            .spawn()
            .chain_err(|| ErrorKind::WindowsUninstallMadness)?;

        // The catch 22 article says we must sleep here to give
        // Windows a chance to bump the processes file reference
        // count. acrichto though is in disbelief and *demanded* that
        // we not insert a sleep. If Windows failed to uninstall
        // correctly it is because of him.

        // (.. and months later acrichto owes me a beer).
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Run by elan-gc-$num.exe to delete ELAN_HOME
#[cfg(windows)]
pub fn complete_windows_uninstall() -> Result<()> {
    use std::ffi::OsStr;
    use std::process::Stdio;

    wait_for_parent()?;

    // Now that the parent has exited there are hopefully no more files open in ELAN_HOME
    let ref elan_home = utils::elan_home()?;
    utils::remove_dir("elan_home", elan_home, &|_| ())?;

    // Now, run a *system* binary to inherit the DELETE_ON_CLOSE
    // handle to *this* process, then exit. The OS will delete the gc
    // exe when it exits.
    let rm_gc_exe = OsStr::new("net");

    Command::new(rm_gc_exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .chain_err(|| ErrorKind::WindowsUninstallMadness)?;

    process::exit(0);
}

#[cfg(windows)]
fn wait_for_parent() -> Result<()> {
    use std::mem;
    use std::ptr;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::processthreadsapi::{GetCurrentProcessId, OpenProcess};
    use winapi::um::synchapi::WaitForSingleObject;
    use winapi::um::tlhelp32::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
    };
    use winapi::um::winbase::{INFINITE, WAIT_OBJECT_0};
    use winapi::um::winnt::SYNCHRONIZE;

    unsafe {
        // Take a snapshot of system processes, one of which is ours
        // and contains our parent's pid
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let _g = scopeguard::guard(snapshot, |h| {
            let _ = CloseHandle(h);
        });

        let mut entry: PROCESSENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;

        // Iterate over system processes looking for ours
        let success = Process32First(snapshot, &mut entry);
        if success == 0 {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let this_pid = GetCurrentProcessId();
        while entry.th32ProcessID != this_pid {
            let success = Process32Next(snapshot, &mut entry);
            if success == 0 {
                let err = io::Error::last_os_error();
                return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
            }
        }

        // FIXME: Using the process ID exposes a race condition
        // wherein the parent process already exited and the OS
        // reassigned its ID.
        let parent_id = entry.th32ParentProcessID;

        // Get a handle to the parent process
        let parent = OpenProcess(SYNCHRONIZE, 0, parent_id);
        if parent == ptr::null_mut() {
            // This just means the parent has already exited.
            return Ok(());
        }

        let _g = scopeguard::guard(parent, |h| {
            let _ = CloseHandle(h);
        });

        // Wait for our parent to exit
        let res = WaitForSingleObject(parent, INFINITE);

        if res != WAIT_OBJECT_0 {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }
    }

    Ok(())
}

#[cfg(unix)]
pub fn complete_windows_uninstall() -> Result<()> {
    panic!("stop doing that")
}

#[derive(PartialEq)]
enum PathUpdateMethod {
    RcFile(PathBuf),
    Windows,
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
fn get_add_path_methods() -> Vec<PathUpdateMethod> {
    if cfg!(windows) {
        return vec![PathUpdateMethod::Windows];
    }

    let profile = utils::home_dir().map(|p| p.join(".profile"));
    let mut profiles = vec![profile];

    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("zsh") {
            let zdotdir = env::var("ZDOTDIR")
                .ok()
                .map(PathBuf::from)
                .or_else(utils::home_dir);
            let zprofile = zdotdir.map(|p| p.join(".zprofile"));
            profiles.push(zprofile);
        }
    }

    if let Some(bash_profile) = utils::home_dir().map(|p| p.join(".bash_profile")) {
        // Only update .bash_profile if it exists because creating .bash_profile
        // will cause .profile to not be read
        if bash_profile.exists() {
            profiles.push(Some(bash_profile));
        }
    }

    let rcfiles = profiles.into_iter().flatten();
    rcfiles.map(PathUpdateMethod::RcFile).collect()
}

fn shell_export_string() -> Result<String> {
    let path = format!("{}/bin", canonical_elan_home()?);
    // The path is *prepended* in case there are system-installed
    // lean's that need to be overridden.
    Ok(format!(r#"export PATH="{}:$PATH""#, path))
}

#[cfg(unix)]
fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {
    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = if rcpath.exists() {
                utils::read_file("rcfile", rcpath)?
            } else {
                String::new()
            };
            let addition = &format!("\n{}", shell_export_string()?);
            if !file.contains(addition) {
                utils::append_file("rcfile", rcpath, addition)?;
            }
        } else {
            unreachable!()
        }
    }

    Ok(())
}

#[cfg(windows)]
fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {
    assert!(methods.len() == 1 && methods[0] == PathUpdateMethod::Windows);

    use std::ptr;
    use winapi::shared::minwindef::*;
    use winapi::um::winuser::{
        SendMessageTimeoutA, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    let old_path = if let Some(s) = get_windows_path_var()? {
        s
    } else {
        // Non-unicode path
        return Ok(());
    };

    let mut new_path = utils::elan_home()?
        .join("bin")
        .to_string_lossy()
        .to_string();
    if old_path.contains(&new_path) {
        return Ok(());
    }

    if !old_path.is_empty() {
        new_path.push_str(";");
        new_path.push_str(&old_path);
    }

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .chain_err(|| ErrorKind::PermissionDenied)?;
    let reg_value = RegValue {
        bytes: utils::string_to_winreg_bytes(&new_path),
        vtype: RegType::REG_EXPAND_SZ,
    };
    environment
        .set_raw_value("PATH", &reg_value)
        .chain_err(|| ErrorKind::PermissionDenied)?;

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0 as WPARAM,
            "Environment\0".as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }

    Ok(())
}

// Get the windows PATH variable out of the registry as a String. If
// this returns None then the PATH varible is not unicode and we
// should not mess with it.
#[cfg(windows)]
fn get_windows_path_var() -> Result<Option<String>> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .chain_err(|| ErrorKind::PermissionDenied)?;

    let reg_value = environment.get_raw_value("PATH");
    match reg_value {
        Ok(val) => {
            if let Some(s) = utils::string_from_winreg_value(&val) {
                Ok(Some(s))
            } else {
                warn!("the registry key HKEY_CURRENT_USER\\Environment\\PATH does not contain valid Unicode. \
                       Not modifying the PATH variable");
                return Ok(None);
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(Some(String::new())),
        Err(e) => Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness),
    }
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
fn get_remove_path_methods() -> Result<Vec<PathUpdateMethod>> {
    if cfg!(windows) {
        return Ok(vec![PathUpdateMethod::Windows]);
    }

    let profile = utils::home_dir().map(|p| p.join(".profile"));
    let bash_profile = utils::home_dir().map(|p| p.join(".bash_profile"));

    let rcfiles = vec![profile, bash_profile];
    let existing_rcfiles = rcfiles.into_iter().flatten().filter(|f| f.exists());

    let export_str = shell_export_string()?;
    let matching_rcfiles = existing_rcfiles.filter(|f| {
        let file = utils::read_file("rcfile", f).unwrap_or_default();
        let addition = &format!("\n{}", export_str);
        file.contains(addition)
    });

    Ok(matching_rcfiles.map(PathUpdateMethod::RcFile).collect())
}

#[cfg(windows)]
fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
    assert!(methods.len() == 1 && methods[0] == PathUpdateMethod::Windows);

    use std::ptr;
    use winapi::shared::minwindef::*;
    use winapi::um::winuser::{
        SendMessageTimeoutA, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    let old_path = if let Some(s) = get_windows_path_var()? {
        s
    } else {
        // Non-unicode path
        return Ok(());
    };

    let ref path_str = utils::elan_home()?
        .join("bin")
        .to_string_lossy()
        .to_string();
    let idx = if let Some(i) = old_path.find(path_str) {
        i
    } else {
        return Ok(());
    };

    // If there's a trailing semicolon (likely, since we added one during install),
    // include that in the substring to remove.
    let mut len = path_str.len();
    if old_path.as_bytes().get(idx + path_str.len()) == Some(&b';') {
        len += 1;
    }

    let mut new_path = old_path[..idx].to_string();
    new_path.push_str(&old_path[idx + len..]);

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .chain_err(|| ErrorKind::PermissionDenied)?;
    if new_path.is_empty() {
        environment
            .delete_value("PATH")
            .chain_err(|| ErrorKind::PermissionDenied)?;
    } else {
        let reg_value = RegValue {
            bytes: utils::string_to_winreg_bytes(&new_path),
            vtype: RegType::REG_EXPAND_SZ,
        };
        environment
            .set_raw_value("PATH", &reg_value)
            .chain_err(|| ErrorKind::PermissionDenied)?;
    }

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0 as WPARAM,
            "Environment\0".as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }

    Ok(())
}

#[cfg(unix)]
fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = utils::read_file("rcfile", rcpath)?;
            let addition = format!("\n{}\n", shell_export_string()?);

            let file_bytes = file.into_bytes();
            let addition_bytes = addition.into_bytes();

            let idx = file_bytes
                .windows(addition_bytes.len())
                .position(|w| w == &*addition_bytes);
            if let Some(i) = idx {
                let mut new_file_bytes = file_bytes[..i].to_vec();
                new_file_bytes.extend(&file_bytes[i + addition_bytes.len()..]);
                let new_file = &String::from_utf8(new_file_bytes).unwrap();
                utils::write_file("rcfile", rcpath, new_file)?;
            } else {
                // Weird case. rcfile no longer needs to be modified?
            }
        } else {
            unreachable!()
        }
    }

    Ok(())
}

/// Self update downloads elan-init to `ELAN_HOME`/bin/elan-init
/// and runs it.
///
/// It does a few things to accomodate self-delete problems on windows:
///
/// elan-init is run in two stages, first with `--self-upgrade`,
/// which displays update messages and asks for confirmations, etc;
/// then with `--self-replace`, which replaces the elan binary and
/// hardlinks. The last step is done without waiting for confirmation
/// on windows so that the running exe can be deleted.
///
/// Because it's again difficult for elan-init to delete itself
/// (and on windows this process will not be running to do it),
/// elan-init is stored in `ELAN_HOME`/bin, and then deleted next
/// time elan runs.
pub fn update() -> Result<()> {
    if elan::install::NEVER_SELF_UPDATE {
        err!("self-update is disabled for this build of elan");
        err!("you should probably use your system package manager to update elan");
        process::exit(1);
    }
    let setup_path = prepare_update()?;
    if let Some(ref p) = setup_path {
        let version = match get_new_elan_version(p) {
            Some(new_version) => parse_new_elan_version(new_version),
            None => {
                err!("failed to get elan version");
                process::exit(1);
            }
        };

        info!("elan updated successfully to {}", version);
        run_update(p)?;
    } else {
        // Try again in case we emitted "tool `{}` is already installed" last time.
        install_proxies()?
    }

    Ok(())
}

fn get_new_elan_version(path: &Path) -> Option<String> {
    match Command::new(path).arg("--version").output() {
        Err(_) => None,
        Ok(output) => match String::from_utf8(output.stdout) {
            Ok(version) => Some(version),
            Err(_) => None,
        },
    }
}

fn parse_new_elan_version(version: String) -> String {
    let re = Regex::new(r"\d+.\d+.\d+[0-9a-zA-Z-]*").unwrap();
    let capture = re.captures(&version);
    let matched_version = match capture {
        Some(cap) => cap.get(0).unwrap().as_str(),
        None => "(unknown)",
    };
    String::from(matched_version)
}

pub fn prepare_update() -> Result<Option<PathBuf>> {
    let elan_home = &(utils::elan_home()?);
    let elan_path = &elan_home.join(format!("bin/elan{}", EXE_SUFFIX));
    let setup_path = &elan_home.join(format!("bin/elan-init{}", EXE_SUFFIX));

    if !elan_path.exists() {
        return Err(ErrorKind::NotSelfInstalled(elan_home.clone()).into());
    }

    if setup_path.exists() {
        utils::remove_file("setup", setup_path)?;
    }

    let update_root = env::var("ELAN_UPDATE_ROOT").unwrap_or(String::from(UPDATE_ROOT));

    let tempdir = tempdir().chain_err(|| "error creating temp directory")?;

    let Some(available_version) = elan::install::check_self_update()? else {
        // If up-to-date
        return Ok(None);
    };

    let archive_suffix = if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    };
    let archive_name = format!("elan-{}{}", dist::host_triple(), archive_suffix);
    let archive_path = tempdir.path().join(&archive_name);
    // Get download URL
    let url = format!("{}/v{}/{}", update_root, available_version, archive_name);

    // Get download path
    let download_url = utils::parse_url(&url)?;

    // Download new version
    info!("downloading self-update");
    utils::download_file(&download_url, &archive_path, &|_| ())?;

    let file = fs::File::open(archive_path)?;
    if cfg!(target_os = "windows") {
        let mut archive =
            zip::read::ZipArchive::new(file).chain_err(|| "failed to open zip archive")?;
        let mut src = archive
            .by_name("elan-init.exe")
            .chain_err(|| "failed to extract update")?;
        let mut dst = fs::File::create(setup_path)?;
        io::copy(&mut src, &mut dst)?;
    } else {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        archive.unpack(elan_home.join("bin"))?;
    }

    // Mark as executable
    utils::make_executable(setup_path)?;

    Ok(Some(setup_path.to_owned()))
}

/// Tell the upgrader to replace the elan bins, then delete
/// itself. Like with uninstallation, on Windows we're going to
/// have to jump through hoops to make everything work right.
///
/// On windows we're not going to wait for it to finish before exiting
/// successfully, so it should not do much, and it should try
/// really hard to succeed, because at this point the upgrade is
/// considered successful.
#[cfg(unix)]
pub fn run_update(setup_path: &Path) -> Result<()> {
    let status = Command::new(setup_path)
        .arg("--self-replace")
        .status()
        .chain_err(|| "unable to run updater")?;

    if !status.success() {
        return Err("self-updated failed to replace elan executable".into());
    }

    process::exit(0);
}

#[cfg(windows)]
pub fn run_update(setup_path: &Path) -> Result<()> {
    Command::new(setup_path)
        .arg("--self-replace")
        .spawn()
        .chain_err(|| "unable to run updater")?;

    process::exit(0);
}

/// This function is as the final step of a self-upgrade. It replaces
/// `ELAN_HOME`/bin/elan with the running exe, and updates the the
/// links to it. On windows this will run *after* the original
/// elan process exits.
#[cfg(unix)]
pub fn self_replace() -> Result<()> {
    install_bins()?;
    clean_up_old_state()?;

    Ok(())
}

#[cfg(windows)]
pub fn self_replace() -> Result<()> {
    wait_for_parent()?;
    install_bins()?;
    clean_up_old_state()?;

    Ok(())
}

pub fn cleanup_self_updater() -> Result<()> {
    let elan_home = utils::elan_home()?;
    let setup = &elan_home.join(format!("bin/elan-init{}", EXE_SUFFIX));

    if setup.exists() {
        utils::remove_file("setup", setup)?;
    }

    // Transitional
    let old_setup = &elan_home.join(format!("bin/multilean-setup{}", EXE_SUFFIX));

    if old_setup.exists() {
        utils::remove_file("setup", old_setup)?;
    }

    Ok(())
}
