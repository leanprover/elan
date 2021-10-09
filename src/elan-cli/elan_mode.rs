use clap::{App, Arg, AppSettings, SubCommand, ArgMatches, Shell};
use common;
use elan::{Cfg, Toolchain, command};
use elan::settings::TelemetryMode;
use errors::*;
use elan_utils::utils;
use self_update;
use std::path::Path;
use std::process::Command;
use std::iter;
use std::error::Error;
use term2;
use std::io::{self, Write};
use help::*;

pub fn main() -> Result<()> {
    try!(::self_update::cleanup_self_updater());

    let ref matches = cli().get_matches();
    let verbose = matches.is_present("verbose");
    let ref cfg = try!(common::set_globals(verbose));

    match matches.subcommand() {
        ("show", Some(_)) => try!(show(cfg)),
        ("install", Some(m)) => try!(update(cfg, m)),
        ("update", Some(m)) => try!(update(cfg, m)),
        ("uninstall", Some(m)) => try!(toolchain_remove(cfg, m)),
        ("default", Some(m)) => try!(default_(cfg, m)),
        ("toolchain", Some(c)) => {
            match c.subcommand() {
                ("install", Some(m)) => try!(update(cfg, m)),
                ("list", Some(_)) => try!(common::list_toolchains(cfg)),
                ("link", Some(m)) => try!(toolchain_link(cfg, m)),
                ("uninstall", Some(m)) => try!(toolchain_remove(cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("override", Some(c)) => {
            match c.subcommand() {
                ("list", Some(_)) => try!(common::list_overrides(cfg)),
                ("set", Some(m)) => try!(override_add(cfg, m)),
                ("unset", Some(m)) => try!(override_remove(cfg, m)),
                (_ ,_) => unreachable!(),
            }
        }
        ("run", Some(m)) => try!(run(cfg, m)),
        ("which", Some(m)) => try!(which(cfg, m)),
        ("doc", Some(m)) => try!(doc(cfg, m)),
        ("man", Some(m)) => try!(man(cfg,m)),
        ("self", Some(c)) => {
            match c.subcommand() {
                ("update", Some(_)) => try!(self_update::update()),
                ("uninstall", Some(m)) => try!(self_uninstall(m)),
                (_ ,_) => unreachable!(),
            }
        }
        ("telemetry", Some(c)) => {
            match c.subcommand() {
                ("enable", Some(_)) => try!(set_telemetry(&cfg, TelemetryMode::On)),
                ("disable", Some(_)) => try!(set_telemetry(&cfg, TelemetryMode::Off)),
                ("analyze", Some(_)) => try!(analyze_telemetry(&cfg)),
                (_, _) => unreachable!(),
            }
        }
        ("completions", Some(c)) => {
            if let Some(shell) = c.value_of("shell") {
                cli().gen_completions_to("elan", shell.parse::<Shell>().unwrap(), &mut io::stdout());
            }
        }
        (_, _) => unreachable!(),
    }

    Ok(())
}

pub fn cli() -> App<'static, 'static> {
    let app = App::new("elan")
        .version(common::version())
        .about("The Lean toolchain installer")
        .after_help(ELAN_HELP)
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(Arg::with_name("verbose")
            .help("Enable verbose output")
            .short("v")
            .long("verbose"))
        .subcommand(SubCommand::with_name("show")
            .about("Show the active and installed toolchains")
            .after_help(SHOW_HELP))
        .subcommand(SubCommand::with_name("install")
            .about("Update Lean toolchains")
            .after_help(INSTALL_HELP)
            .setting(AppSettings::Hidden) // synonym for 'toolchain install'
            .arg(Arg::with_name("toolchain")
                .help(TOOLCHAIN_ARG_HELP)
                .required(true)
                .multiple(true)))
        .subcommand(SubCommand::with_name("uninstall")
            .about("Uninstall Lean toolchains")
            .setting(AppSettings::Hidden) // synonym for 'toolchain uninstall'
            .arg(Arg::with_name("toolchain")
                .help(TOOLCHAIN_ARG_HELP)
                .required(true)
                .multiple(true)))
        .subcommand(SubCommand::with_name("update")
            .about("Update Lean toolchains and elan")
            .after_help(UPDATE_HELP)
            .arg(Arg::with_name("toolchain")
                .help(TOOLCHAIN_ARG_HELP)
                .required(false)
                .multiple(true))
            .arg(Arg::with_name("no-self-update")
                .help("Don't perform self update when running the `elan` command")
                .long("no-self-update")
                .takes_value(false)
                .hidden(true))
            .arg(Arg::with_name("force")
                .help("Force an update, even if some components are missing")
                .long("force")
                .takes_value(false)))
        .subcommand(SubCommand::with_name("default")
            .about("Set the default toolchain")
            .after_help(DEFAULT_HELP)
            .arg(Arg::with_name("toolchain")
                .help(TOOLCHAIN_ARG_HELP)
                .required(true)))
        .subcommand(SubCommand::with_name("toolchain")
            .about("Modify or query the installed toolchains")
            .after_help(TOOLCHAIN_HELP)
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("list")
                .about("List installed toolchains"))
            .subcommand(SubCommand::with_name("install")
                .about("Install or update a given toolchain")
                .aliases(&["update", "add"])
                .arg(Arg::with_name("toolchain")
                     .help(TOOLCHAIN_ARG_HELP)
                     .required(true)
                     .multiple(true)))
            .subcommand(SubCommand::with_name("uninstall")
                .about("Uninstall a toolchain")
                .alias("remove")
                .arg(Arg::with_name("toolchain")
                     .help(TOOLCHAIN_ARG_HELP)
                     .required(true)
                     .multiple(true)))
            .subcommand(SubCommand::with_name("link")
                .about("Create a custom toolchain by symlinking to a directory")
                .after_help(TOOLCHAIN_LINK_HELP)
                .arg(Arg::with_name("toolchain")
                    .help(TOOLCHAIN_ARG_HELP)
                    .required(true))
                .arg(Arg::with_name("path")
                    .required(true))))
        .subcommand(SubCommand::with_name("override")
            .about("Modify directory toolchain overrides")
            .after_help(OVERRIDE_HELP)
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("list")
                .about("List directory toolchain overrides"))
            .subcommand(SubCommand::with_name("set")
                .about("Set the override toolchain for a directory")
                .alias("add")
                .arg(Arg::with_name("toolchain")
                     .help(TOOLCHAIN_ARG_HELP)
                     .required(true)))
            .subcommand(SubCommand::with_name("unset")
                .about("Remove the override toolchain for a directory")
                .after_help(OVERRIDE_UNSET_HELP)
                .alias("remove")
                .arg(Arg::with_name("path")
                    .long("path")
                    .takes_value(true)
                    .help("Path to the directory"))
                .arg(Arg::with_name("nonexistent")
                    .long("nonexistent")
                    .takes_value(false)
                    .help("Remove override toolchain for all nonexistent directories"))))
        .subcommand(SubCommand::with_name("run")
            .about("Run a command with an environment configured for a given toolchain")
            .after_help(RUN_HELP)
            .setting(AppSettings::TrailingVarArg)
            .arg(Arg::with_name("install")
                .help("Install the requested toolchain if needed")
                .long("install"))
            .arg(Arg::with_name("toolchain")
                .help(TOOLCHAIN_ARG_HELP)
                .required(true))
            .arg(Arg::with_name("command")
                .required(true).multiple(true).use_delimiter(false)))
        .subcommand(SubCommand::with_name("which")
            .about("Display which binary will be run for a given command")
            .arg(Arg::with_name("command")
                .required(true)))
        /*.subcommand(SubCommand::with_name("doc")
            .alias("docs")
            .about("Open the documentation for the current toolchain")
            .after_help(DOC_HELP)
            .arg(Arg::with_name("book")
                 .long("book")
                 .help("The Rust Programming Language book"))
            .arg(Arg::with_name("std")
                 .long("std")
                 .help("Standard library API documentation"))
            .group(ArgGroup::with_name("page")
                 .args(&["book", "std"])))*/;

    /*if cfg!(not(target_os = "windows")) {
        app = app
            .subcommand(SubCommand::with_name("man")
                    .about("View the man page for a given command")
                    .arg(Arg::with_name("command")
                         .required(true))
                    .arg(Arg::with_name("toolchain")
                         .help(TOOLCHAIN_ARG_HELP)
                         .long("toolchain")
                         .takes_value(true)));
    }*/

    app.subcommand(SubCommand::with_name("self")
        .about("Modify the elan installation")
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("update")
            .about("Download and install updates to elan"))
        .subcommand(SubCommand::with_name("uninstall")
            .about("Uninstall elan.")
            .arg(Arg::with_name("no-prompt")
                    .short("y"))))
    /*.subcommand(SubCommand::with_name("telemetry")
        .about("elan telemetry commands")
        .setting(AppSettings::Hidden)
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("enable")
                        .about("Enable elan telemetry"))
        .subcommand(SubCommand::with_name("disable")
                        .about("Disable elan telemetry"))
        .subcommand(SubCommand::with_name("analyze")
                        .about("Analyze stored telemetry")))*/
    .subcommand(SubCommand::with_name("completions")
        .about("Generate completion scripts for your shell")
        .after_help(COMPLETIONS_HELP)
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(Arg::with_name("shell")
            .possible_values(&Shell::variants())))
}

fn default_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let ref toolchain = try!(cfg.get_toolchain(toolchain, false));

    let status = if !toolchain.exists() || !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist_if_not_installed()))
    } else {
        None
    };

    try!(toolchain.make_default());

    if let Some(status) = status {
        println!("");
        try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn update(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if let Some(names) = m.values_of("toolchain") {
        for name in names {
            let toolchain = try!(cfg.get_toolchain(name, false));

            let status = if !toolchain.exists() || !toolchain.is_custom() {
                Some(try!(toolchain.install_from_dist(m.is_present("force"))))
            } else {
                None
            };

            if let Some(status) = status {
                println!("");
                try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
            }
        }
    } else {
        try!(common::update_all_channels(
            cfg,
            !m.is_present("no-self-update") && !self_update::NEVER_SELF_UPDATE,
            m.is_present("force"),
        ));
    }

    Ok(())
}

fn run(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let cmd = try!(cfg.create_command_for_toolchain(toolchain, m.is_present("install"), args[0]));

    Ok(try!(command::run_command_for_dir(cmd, args[0], &args[1..], &cfg)))
}

fn which(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let binary = m.value_of("command").expect("");

    let binary_path = try!(cfg.which_binary(&try!(utils::current_dir()), binary))
                          .expect("binary not found");

    try!(utils::assert_is_file(&binary_path));

    println!("{}", binary_path.display());

    Ok(())
}

fn show(cfg: &Cfg) -> Result<()> {
    let ref cwd = try!(utils::current_dir());
    let installed_toolchains = try!(cfg.list_toolchains());
    let active_toolchain = cfg.find_override_toolchain_or_default(cwd);

    let show_installed_toolchains = installed_toolchains.len() > 1;
    let show_active_toolchain = true;

    // Only need to display headers if we have multiple sections
    let show_headers = [
        show_installed_toolchains,
        show_active_toolchain
    ].iter().filter(|x| **x).count() > 1;

    if show_installed_toolchains {
        if show_headers { print_header("installed toolchains") }
        let default_name = try!(cfg.get_default());
        for t in installed_toolchains {
            if default_name.as_ref() == Some(&t) {
                println!("{} (default)", t);
            } else {
                println!("{}", t);
            }
        }
        if show_headers { println!("") };
    }

    if show_active_toolchain {
        if show_headers { print_header("active toolchain") }

        match active_toolchain {
            Ok(atc) => {
                match atc {
                    Some((ref toolchain, Some(ref reason))) => {
                        println!("{} ({})", toolchain.name(), reason);
                        println!("{}", common::lean_version(toolchain));
                    }
                    Some((ref toolchain, None)) => {
                        println!("{} (default)", toolchain.name());
                        println!("{}", common::lean_version(toolchain));
                    }
                    None => {
                        println!("no active toolchain");
                    }
                }
            }
            Err(err) => {
                if let Some(cause) = err.cause() {
                    println!("(error: {}, {})", err, cause);
                } else {
                    println!("(error: {})", err);
                }
            }
        }

        if show_headers { println!("") };
    }

    fn print_header(s: &str) {
        let mut t = term2::stdout();
        let _ = t.attr(term2::Attr::Bold);
        let _ = writeln!(t, "{}", s);
        let _ = writeln!(t, "{}", iter::repeat("-").take(s.len()).collect::<String>());
        let _ = writeln!(t, "");
        let _ = t.reset();
    }

    Ok(())
}

fn explicit_or_dir_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches) -> Result<Toolchain<'a>> {
    let toolchain = m.value_of("toolchain");
    if let Some(toolchain) = toolchain {
        let toolchain = try!(cfg.get_toolchain(toolchain, false));
        return Ok(toolchain);
    }

    let ref cwd = try!(utils::current_dir());
    let (toolchain, _) = try!(cfg.toolchain_for_dir(cwd));

    Ok(toolchain)
}

fn toolchain_link(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let ref path = m.value_of("path").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, true));

    Ok(try!(toolchain.install_from_dir(Path::new(path), true)))
}

fn toolchain_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    for toolchain in m.values_of("toolchain").expect("") {
        let toolchain = try!(cfg.get_toolchain(toolchain, false));
        try!(toolchain.remove());
    }
    Ok(())
}

fn override_add(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, false));

    let status = if !toolchain.exists() || !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist_if_not_installed()))
    } else {
        None
    };

    try!(toolchain.make_override(&try!(utils::current_dir())));

    if let Some(status) = status {
        println!("");
        try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn override_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let paths = if m.is_present("nonexistent") {
        let list: Vec<_> = try!(cfg.settings_file.with(|s| Ok(s.overrides.iter().filter_map(|(k, _)|
            if Path::new(k).is_dir() {
                None
            } else {
                Some(k.clone())
            }
        ).collect())));
        if list.is_empty() {
            info!("no nonexistent paths detected");
        }
        list
    } else {
        if m.is_present("path") {
            vec![m.value_of("path").unwrap().to_string()]
        } else {
            vec![try!(utils::current_dir()).to_str().unwrap().to_string()]
        }
    };

    for path in paths {
        if try!(cfg.settings_file.with_mut(|s| {
            Ok(s.remove_override(&Path::new(&path), cfg.notify_handler.as_ref()))
        })) {
            info!("override toolchain for '{}' removed", path);
        } else {
            info!("no override toolchain for '{}'", path);
            if !m.is_present("path") && !m.is_present("nonexistent") {
                info!("you may use `--path <path>` option to remove override toolchain \
                       for a specific path");
            }
        }
    }
    Ok(())
}

fn doc(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let doc_url = if m.is_present("book") {
        "book/index.html"
    } else if m.is_present("std") {
        "std/index.html"
    } else {
        "index.html"
    };

    Ok(try!(cfg.open_docs_for_dir(&try!(utils::current_dir()), doc_url)))
}

fn man(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let manpage = m.value_of("command").expect("");
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));
    let mut man_path = toolchain.path().to_path_buf();
    man_path.push("share");
    man_path.push("man");
    man_path.push("man1");
    man_path.push(manpage.to_owned() + ".1");
    try!(utils::assert_is_file(&man_path));
    Command::new("man")
        .arg(man_path)
        .status()
        .expect("failed to open man page");
    Ok(())
}

fn self_uninstall(m: &ArgMatches) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");

    self_update::uninstall(no_prompt)
}

fn set_telemetry(cfg: &Cfg, t: TelemetryMode) -> Result<()> {
    match t {
        TelemetryMode::On => Ok(try!(cfg.set_telemetry(true))),
        TelemetryMode::Off => Ok(try!(cfg.set_telemetry(false))),
    }
}

fn analyze_telemetry(cfg: &Cfg) -> Result<()> {
    let analysis = try!(cfg.analyze_telemetry());
    common::show_telemetry(analysis)
}
