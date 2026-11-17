use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOLS: [&str; 2] = ["lean-beam", "lean-beam-mcp"];

struct Installation {
    root: tempfile::TempDir,
}

impl Installation {
    fn new() -> Self {
        let installation = Self {
            root: tempfile::Builder::new()
                .prefix("elan beam proxies ")
                .tempdir()
                .unwrap(),
        };
        installation.install();
        installation
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("elan")
    }

    fn binary(&self, name: &str) -> PathBuf {
        self.home()
            .join("bin")
            .join(format!("{}{}", name, std::env::consts::EXE_SUFFIX))
    }

    fn command(&self, program: impl AsRef<Path>) -> Command {
        let mut command = Command::new(program.as_ref());
        command
            .env("ELAN_HOME", self.home())
            .env("HOME", self.root.path())
            .env("USERPROFILE", self.root.path())
            .env_remove("ELAN_TOOLCHAIN")
            .env_remove("LEAN_RECURSION_COUNT")
            .current_dir(self.root.path());
        command
    }

    fn install(&self) {
        success(self.command(env!("CARGO_BIN_EXE_elan-init")).args([
            "-y",
            "--default-toolchain",
            "none",
            "--no-modify-path",
        ]));
    }

    fn sdk(&self, name: &str) -> PathBuf {
        let root = self.root.path().join("sdks").join(name);
        fs::create_dir_all(root.join("bin")).unwrap();
        // Linking checks for Lean's presence; these fixtures never execute it.
        fs::write(
            root.join("bin")
                .join(format!("lean{}", std::env::consts::EXE_SUFFIX)),
            b"fixture",
        )
        .unwrap();
        success(
            self.command(self.binary("elan"))
                .args(["toolchain", "link", name])
                .arg(&root),
        );
        root
    }
}

fn success(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{:?}: {:?}", command, output);
    output
}

#[test]
fn installation_and_repair_create_beam_proxies() {
    let installation = Installation::new();
    let check = || {
        for tool in TOOLS {
            assert!(same_file::is_same_file(
                installation.binary("elan"),
                installation.binary(tool)
            )
            .unwrap());
        }
    };
    check();
    fs::remove_file(installation.binary("lean-beam-mcp")).unwrap();
    // Reinstallation also replaces the existing proxy's link to the old Elan binary.
    installation.install();
    check();
}

#[test]
fn beam_proxies_select_linked_project_and_explicit_toolchains() {
    let installation = Installation::new();
    installation.sdk("beam-a");
    installation.sdk("beam-b");
    fs::write(installation.root.path().join("lean-toolchain"), "beam-a\n").unwrap();
    let project = installation.root.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("lean-toolchain"), "beam-b\n").unwrap();
    for tool in TOOLS {
        for (explicit, inherited, selected) in [
            (false, None, "beam-b"),
            (false, Some("beam-a"), "beam-a"),
            (true, Some("beam-b"), "beam-a"),
        ] {
            let mut command = installation.command(installation.binary(tool));
            command.current_dir(&project);
            if let Some(inherited) = inherited {
                command.env("ELAN_TOOLCHAIN", inherited);
            }
            if explicit {
                command.arg("+beam-a");
            }
            let output = command.output().unwrap();
            assert!(!output.status.success(), "{:?}", output);
            assert!(output.stdout.is_empty(), "{:?}", output);
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(
                error.contains(&format!(
                    "toolchain '{}' does not have the binary",
                    selected
                )),
                "{}",
                error
            );
            assert!(
                error.contains(&format!("{}{}", tool, std::env::consts::EXE_SUFFIX)),
                "{}",
                error
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn mcp_proxy_preserves_stdio_and_uses_the_project_sdk() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;

    let installation = Installation::new();
    for name in ["beam-a", "beam-b"] {
        let sdk = installation.sdk(name);
        let tool = sdk.join("bin/lean-beam-mcp");
        fs::write(
            &tool,
            format!("#!/bin/sh\nprintf '%s\\n' '{name}' \"$ELAN_TOOLCHAIN\" \"$@\" >&2\ncat\n"),
        )
        .unwrap();
        fs::set_permissions(tool, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n";
    for name in ["beam-a", "beam-b"] {
        let sdk = fs::canonicalize(installation.root.path().join("sdks").join(name)).unwrap();
        for selector in [name.to_owned(), sdk.display().to_string()] {
            fs::write(installation.root.path().join("lean-toolchain"), &selector).unwrap();
            let mut child = installation
                .command(installation.binary("lean-beam-mcp"))
                .args(["--fixture", "argument with spaces"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(input).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "{:?}", output);
            assert_eq!(output.stdout, input);
            assert_eq!(
                String::from_utf8(output.stderr).unwrap(),
                format!("{name}\n{selector}\n--fixture\nargument with spaces\n")
            );
        }
    }
}
