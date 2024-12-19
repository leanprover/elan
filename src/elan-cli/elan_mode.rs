use crate::common;
use crate::errors::*;
use crate::help::*;
use crate::self_update;
use crate::term2;
use clap::{App, AppSettings, Arg, ArgMatches, Shell, SubCommand};
use elan::{command, gc, lookup_toolchain_desc, lookup_unresolved_toolchain_desc, Cfg, Toolchain};
use elan_dist::dist::ToolchainDesc;
use elan_utils::utils;
use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use serde_derive::Serialize;

use crate::json_dump;

pub fn main() -> Result<()> {
    crate::self_update::cleanup_self_updater()?;

    let matches = &cli().get_matches();
    let verbose = matches.is_present("verbose");
    let cfg = &(common::set_globals(verbose)?);

    match matches.subcommand() {
        ("show", Some(_)) => show(cfg)?,
        ("install", Some(m)) => install(cfg, m)?,
        ("uninstall", Some(m)) => toolchain_remove(cfg, m)?,
        ("default", Some(m)) => default_(cfg, m)?,
        ("toolchain", Some(c)) => match c.subcommand() {
            ("install", Some(m)) => install(cfg, m)?,
            ("list", Some(_)) => list_toolchains(cfg)?,
            ("link", Some(m)) => toolchain_link(cfg, m)?,
            ("uninstall", Some(m)) => toolchain_remove(cfg, m)?,
            ("gc", Some(m)) => toolchain_gc(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("override", Some(c)) => match c.subcommand() {
            ("list", Some(_)) => common::list_overrides(cfg)?,
            ("set", Some(m)) => override_add(cfg, m)?,
            ("unset", Some(m)) => override_remove(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("run", Some(m)) => run(cfg, m)?,
        ("which", Some(m)) => which(cfg, m)?,
        ("doc", Some(m)) => doc(cfg, m)?,
        ("man", Some(m)) => man(cfg, m)?,
        ("self", Some(c)) => match c.subcommand() {
            ("update", Some(_)) => self_update::update()?,
            ("uninstall", Some(m)) => self_uninstall(m)?,
            (_, _) => unreachable!(),
        },
        ("completions", Some(c)) => {
            if let Some(shell) = c.value_of("shell") {
                cli().gen_completions_to(
                    "elan",
                    shell.parse::<Shell>().unwrap(),
                    &mut io::stdout(),
                );
            }
        }
        ("dump-state", Some(m)) => dump_state(cfg, m)?,
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
            .about("Install Lean toolchain")
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
                .about("Install a given toolchain")
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
                    .required(true)))
            .subcommand(SubCommand::with_name("gc")
                .about("Garbage-collect toolchains not used by any known project")
                .after_help(TOOLCHAIN_GC_HELP)
                .arg(Arg::with_name("delete")
                    .long("delete")
                    .help("Delete collected toolchains instead of only reporting them"))
                .arg(Arg::with_name("json")
                    .long("json")
                    .help("Format output as JSON"))))
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
        .subcommand(SubCommand::with_name("dump-state")
            .setting(AppSettings::Hidden)
            .arg(Arg::with_name("no-net")
                .help("Make network operations for resolving channels fail immediately")
                .long("no-net")))
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

    app.subcommand(
        SubCommand::with_name("self")
            .about("Modify the elan installation")
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(
                SubCommand::with_name("update").about("Download and install updates to elan"),
            )
            .subcommand(
                SubCommand::with_name("uninstall")
                    .about("Uninstall elan.")
                    .arg(Arg::with_name("no-prompt").short("y")),
            ),
    )
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
    .subcommand(
        SubCommand::with_name("completions")
            .about("Generate completion scripts for your shell")
            .after_help(COMPLETIONS_HELP)
            .setting(AppSettings::ArgRequiredElseHelp)
            .arg(Arg::with_name("shell").possible_values(&Shell::variants())),
    )
}

fn default_(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let name = m.value_of("toolchain").expect("");
    // sanity-check
    let _ = lookup_unresolved_toolchain_desc(cfg, name)?;

    cfg.set_default(name)?;
    Ok(())
}

fn install(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let names = m.values_of("toolchain").expect("");
    for name in names {
        let desc = lookup_toolchain_desc(cfg, name)?;
        let toolchain = cfg.get_toolchain(&desc, false)?;

        if !toolchain.exists() || !toolchain.is_custom() {
            toolchain.install_from_dist()?;
            println!();
            common::show_channel_update(cfg, &toolchain.desc)?;
        }
    }

    Ok(())
}

fn run(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = m.value_of("toolchain").expect("");
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let desc = lookup_toolchain_desc(cfg, toolchain)?;
    let cmd = cfg.create_command_for_toolchain(&desc, m.is_present("install"), args[0])?;

    Ok(command::run_command_for_dir(cmd, args[0], &args[1..])?)
}

fn which(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let binary = m.value_of("command").expect("");

    let binary_path = cfg
        .which_binary(&utils::current_dir()?, binary)?
        .expect("binary not found");

    utils::assert_is_file(&binary_path)?;

    println!("{}", binary_path.display());

    Ok(())
}

pub fn mk_toolchain_label(
    tc: &ToolchainDesc,
    default_tc: &Option<String>,
    resolved_default_tc: &Option<ToolchainDesc>,
) -> String {
    let resolved_default_str = resolved_default_tc.as_ref().map(|tc| tc.to_string());
    if resolved_default_str.as_ref() == Some(&tc.to_string()) {
        if &resolved_default_str == default_tc {
            format!("{} (default)", tc)
        } else {
            format!(
                "{} (resolved from default '{}')",
                tc,
                default_tc.as_ref().unwrap()
            )
        }
    } else {
        format!("{}", tc)
    }
}

pub fn list_toolchains(cfg: &Cfg) -> Result<()> {
    let toolchains = cfg.list_toolchains()?;

    if toolchains.is_empty() {
        println!("no installed toolchains");
    } else {
        for tc in toolchains {
            println!("{}", tc);
        }
    }
    Ok(())
}

fn show(cfg: &Cfg) -> Result<()> {
    let cwd = &(utils::current_dir()?);
    let installed_toolchains = cfg.list_toolchains()?;
    let active_toolchain = cfg.find_override_toolchain_or_default(cwd);

    let show_installed_toolchains = installed_toolchains.len() > 1;
    let show_active_toolchain = true;

    // Only need to display headers if we have multiple sections
    let show_headers = [show_installed_toolchains, show_active_toolchain]
        .iter()
        .filter(|x| **x)
        .count()
        > 1;

    let default_tc = cfg.get_default()?;
    let resolved_default_tc = default_tc
        .as_ref()
        .map(|tc| lookup_toolchain_desc(cfg, tc))
        .transpose()?;
    if show_installed_toolchains {
        if show_headers {
            print_header("installed toolchains")
        }
        for t in installed_toolchains {
            println!(
                "{}",
                mk_toolchain_label(&t, &default_tc, &resolved_default_tc)
            );
        }
        if show_headers {
            println!()
        };
    }

    if show_active_toolchain {
        if show_headers {
            print_header("active toolchain")
        }

        match active_toolchain {
            Ok(atc) => match atc {
                Some((ref toolchain, Some(ref reason))) => {
                    println!("{} ({})", toolchain.name(), reason);
                    println!("{}", common::lean_version(toolchain));
                }
                Some((ref toolchain, None)) => {
                    println!(
                        "{}",
                        mk_toolchain_label(&toolchain.desc, &default_tc, &resolved_default_tc)
                    );
                    println!("{}", common::lean_version(toolchain));
                }
                None => {
                    println!("no active toolchain");
                }
            },
            Err(err) => {
                if let Some(cause) = err.source() {
                    println!("(error: {}, {})", err, cause);
                } else {
                    println!("(error: {})", err);
                }
            }
        }

        if show_headers {
            println!()
        };
    }

    fn print_header(s: &str) {
        let mut t = term2::stdout();
        let _ = t.attr(term2::Attr::Bold);
        let _ = writeln!(t, "{}", s);
        let _ = writeln!(t, "{}", "-".repeat(s.len()));
        let _ = writeln!(t);
        let _ = t.reset();
    }

    Ok(())
}

fn explicit_or_dir_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches<'_>) -> Result<Toolchain<'a>> {
    let toolchain = m.value_of("toolchain");
    if let Some(toolchain) = toolchain {
        let desc = lookup_toolchain_desc(cfg, toolchain)?;
        let toolchain = cfg.get_toolchain(&desc, false)?;
        return Ok(toolchain);
    }

    let cwd = &(utils::current_dir()?);
    let (toolchain, _) = cfg.toolchain_for_dir(cwd)?;

    Ok(toolchain)
}

fn toolchain_link(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = &m.value_of("toolchain").expect("");
    let path = &m.value_of("path").expect("");
    let desc = ToolchainDesc::Local {
        name: toolchain.to_string(),
    };
    let toolchain = cfg.get_toolchain(&desc, true)?;

    Ok(toolchain.install_from_dir(Path::new(path), true)?)
}

fn toolchain_remove(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    for toolchain in m.values_of("toolchain").expect("") {
        let desc = lookup_toolchain_desc(cfg, toolchain)?;
        let toolchain = cfg.get_toolchain(&desc, false)?;
        toolchain.remove()?;
    }
    Ok(())
}

#[derive(Serialize)]
struct UsedToolchain {
    // project root or "default toolchain"
    user: String,
    toolchain: String,
}

#[derive(Serialize)]
struct GCResult {
    unused_toolchains: Vec<String>,
    used_toolchains: Vec<UsedToolchain>,
}

fn toolchain_gc(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let (unused_toolchains, used_toolchains) = gc::analyze_toolchains(cfg)?;
    let delete = m.is_present("delete");
    let json = m.is_present("json");
    if json {
        let result = GCResult {
            unused_toolchains: unused_toolchains
                .iter()
                .map(|t| t.desc.to_string())
                .collect(),
            used_toolchains: used_toolchains
                .iter()
                .map(|(root, tc)| UsedToolchain {
                    user: root.clone(),
                    toolchain: tc.to_string(),
                })
                .collect(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&result).chain_err(|| "failed to print JSON")?
        );
        return Ok(());
    }

    if unused_toolchains.is_empty() {
        println!("No unused toolchains found");
    } else {
        if !delete {
            println!("The following toolchains are not used by any known project; rerun with `--delete` to delete them:");
        }
        for t in unused_toolchains.into_iter() {
            if delete {
                t.remove()?;
            } else {
                println!("- {}", t.desc);
            }
        }
    }
    if !delete {
        println!("Known projects:");
        for (root, tc) in used_toolchains.into_iter() {
            println!("- {}: {}", root, tc);
        }
    }
    Ok(())
}

fn override_add(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = m.value_of("toolchain").expect("");
    let desc = lookup_toolchain_desc(cfg, toolchain)?;
    let toolchain = cfg.get_toolchain(&desc, false)?;
    toolchain.make_override(&utils::current_dir()?)?;
    Ok(())
}

fn override_remove(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let paths = if m.is_present("nonexistent") {
        let list: Vec<_> = cfg.settings_file.with(|s| {
            Ok(s.overrides
                .iter()
                .filter_map(|(k, _)| {
                    if Path::new(k).is_dir() {
                        None
                    } else {
                        Some(k.clone())
                    }
                })
                .collect())
        })?;
        if list.is_empty() {
            info!("no nonexistent paths detected");
        }
        list
    } else if m.is_present("path") {
        vec![m.value_of("path").unwrap().to_string()]
    } else {
        vec![utils::current_dir()?.to_str().unwrap().to_string()]
    };

    for path in paths {
        if cfg
            .settings_file
            .with_mut(|s| Ok(s.remove_override(Path::new(&path), cfg.notify_handler.as_ref())))?
        {
            info!("override toolchain for '{}' removed", path);
        } else {
            info!("no override toolchain for '{}'", path);
            if !m.is_present("path") && !m.is_present("nonexistent") {
                info!(
                    "you may use `--path <path>` option to remove override toolchain \
                       for a specific path"
                );
            }
        }
    }
    Ok(())
}

fn doc(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let doc_url = if m.is_present("book") {
        "book/index.html"
    } else if m.is_present("std") {
        "std/index.html"
    } else {
        "index.html"
    };

    Ok(cfg.open_docs_for_dir(&utils::current_dir()?, doc_url)?)
}

fn man(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let manpage = m.value_of("command").expect("");
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let mut man_path = toolchain.path().to_path_buf();
    man_path.push("share");
    man_path.push("man");
    man_path.push("man1");
    man_path.push(manpage.to_owned() + ".1");
    utils::assert_is_file(&man_path)?;
    Command::new("man")
        .arg(man_path)
        .status()
        .expect("failed to open man page");
    Ok(())
}

fn self_uninstall(m: &ArgMatches<'_>) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");

    self_update::uninstall(no_prompt)
}

fn dump_state(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let no_net = m.is_present("no-net");

    Ok(json_dump::StateDump::new(cfg, no_net)?.print()?)
}
