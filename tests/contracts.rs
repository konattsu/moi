struct Fixture {
    root: tempfile::TempDir,
    home: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::create_dir_all(root.path().join("environments/test/modules")).unwrap();
        Self { root, home }
    }
    fn module(&self, name: &str, body: &str) -> std::path::PathBuf {
        let path = self
            .root
            .path()
            .join("environments/test/modules")
            .join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("module.toml"), format!("name = {name:?}\n{body}"))
            .unwrap();
        path
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_moi"))
            .args([
                "--source",
                url::Url::from_directory_path(self.root.path())
                    .unwrap()
                    .as_str(),
                "-e",
                "test",
            ])
            .args(args)
            .env("HOME", &self.home)
            .env_remove("MOI_SOURCE")
            .env_remove("MOI_FOLDER_NAME")
            .env_remove("MOI_ENVIRONMENT")
            .env_remove("MOI_FIRST_INSTALL")
            .env_remove("MOI_BRANCH")
            .output()
            .unwrap()
    }
    fn success(&self, args: &[&str]) -> std::process::Output {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

#[test]
fn plan_never_executes_scripts_or_writes_configuration() {
    let f = Fixture::new();
    f.module(
        "a",
        r#"
note = ["show this note"]
[env]
path_export = 'touch "$HOME/exported"; printf "%s\0" "$PATH"'
[[operations]]
kind = "shell"
check = 'touch "$HOME/checked"; exit 1'
run = 'touch "$HOME/ran"'
[[operations]]
kind = "directory"
path = "~/directory"
"#,
    );
    let out = f.success(&["plan", "--show-note"]);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PATH obtained during apply")
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("show this note"));
    assert_eq!(std::fs::read_dir(&f.home).unwrap().count(), 0);
}

#[test]
fn file_converges_preserves_unspecified_permissions_and_does_not_rewrite() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let m = f.module(
        "a",
        r#"[[operations]]
kind = "file"
src = "source"
dst = "~/result"
"#,
    );
    std::fs::write(m.join("source"), "desired").unwrap();
    let dst = f.home.join("result");
    std::fs::write(&dst, "old").unwrap();
    std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o600)).unwrap();
    let first = f.success(&["apply"]);
    assert!(String::from_utf8_lossy(&first.stderr).contains("changed=1"));
    let meta = std::fs::metadata(&dst).unwrap();
    assert_eq!(meta.permissions().mode() & 0o7777, 0o600);
    let modified = meta.modified().unwrap();
    let second = f.success(&["apply"]);
    assert!(String::from_utf8_lossy(&second.stderr).contains("unchanged=1"));
    assert_eq!(
        std::fs::metadata(dst).unwrap().modified().unwrap(),
        modified
    );
    assert!(!f.home.join(".config/moi/config.toml").exists());
}

#[test]
fn operations_execute_in_declared_order_and_recheck_shell() {
    let f = Fixture::new();
    f.module(
        "a",
        r#"[[operations]]
kind = "shell"
check = 'test -f "$HOME/first"'
run = 'touch "$HOME/first"'
[[operations]]
kind = "directory"
path = "~/second"
[[operations]]
kind = "shell"
run = 'test -f "$HOME/first"; test -d "$HOME/second"; touch "$HOME/third"'
"#,
    );
    let out = f.success(&["apply"]);
    assert!(f.home.join("third").exists());
    assert!(String::from_utf8_lossy(&out.stderr).contains("executed=2"));
    let f = Fixture::new();
    f.module(
        "a",
        r#"[[operations]]
kind = "shell"
check = "exit 1"
run = "true"
[[operations]]
kind = "directory"
path = "~/later"
"#,
    );
    let out = f.run(&["apply"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("post-check"));
    assert!(!f.home.join("later").exists());
}

#[test]
fn check_errors_do_not_run_and_change_errors_keep_child_exit_code() {
    let f = Fixture::new();
    f.module(
        "a",
        r#"[[operations]]
kind = "shell"
check = "echo judgment-error >&2; exit 42"
run = 'touch "$HOME/ran"'
"#,
    );
    let out = f.run(&["apply"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!f.home.join("ran").exists());
    assert!(String::from_utf8_lossy(&out.stderr).contains("judgment-error"));
    f.module(
        "a",
        r#"[[operations]]
kind = "shell"
run = "echo change-failed; exit 37"
"#,
    );
    let out = f.run(&["apply", "--quiet"]);
    assert_eq!(out.status.code(), Some(37));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("change-failed")
            && err.contains("operations[1] shell")
            && err.contains("run")
    );
    assert!(out.stdout.is_empty());
}

#[test]
fn preflight_rejects_late_missing_input_before_any_change() {
    let f = Fixture::new();
    f.module(
        "a",
        r#"[[operations]]
kind = "shell"
run = 'touch "$HOME/changed"'
[[operations]]
kind = "file"
src = "missing"
dst = "~/result"
"#,
    );
    assert!(!f.run(&["apply"]).status.success());
    assert!(!f.home.join("changed").exists());
}

#[test]
fn unselected_definitions_are_validated_but_unselected_inputs_are_not_required() {
    let f = Fixture::new();
    f.module("a", "");
    f.module(
        "b",
        r#"[[operations]]
kind = "file"
src = "missing"
dst = "~/unused"
"#,
    );
    f.success(&["plan", "a"]);
    f.module("b", "depends_on = [\"missing\"]");
    assert!(!f.run(&["plan", "a"]).status.success());
    f.module("b", "unknown = true");
    assert!(!f.run(&["plan", "a"]).status.success());
}

#[test]
fn conflicts_and_symlink_parents_are_rejected_before_changes() {
    let f = Fixture::new();
    let m = f.module(
        "a",
        r#"[[operations]]
kind = "directory"
path = "~/before"
[[operations]]
kind = "file"
src = "source"
dst = "~/same"
[[operations]]
kind = "block"
src = "source"
dst = "~/./same/"
marker = "moi:a"
"#,
    );
    std::fs::write(m.join("source"), "data").unwrap();
    assert!(!f.run(&["apply"]).status.success());
    assert!(!f.home.join("before").exists());
    let f = Fixture::new();
    std::fs::create_dir(f.home.join("real")).unwrap();
    std::os::unix::fs::symlink(f.home.join("real"), f.home.join("link")).unwrap();
    f.module(
        "a",
        r#"[[operations]]
kind = "directory"
path = "~/link/child"
"#,
    );
    assert!(!f.run(&["apply"]).status.success());
    assert!(!f.home.join("real/child").exists());
}

#[test]
fn path_export_runs_for_empty_modules_and_propagates_without_tool_special_cases() {
    let f = Fixture::new();
    let bin = f.home.join("tools");
    std::fs::create_dir(&bin).unwrap();
    f.module(
        "provider",
        r#"[env]
path_prepend = ["~/tools", "~/tools"]
path_export = '''
test "${PATH%%:*}" = "$HOME/tools"
printf '%s\0' "$PATH"
'''
"#,
    );
    f.module(
        "consumer",
        r#"depends_on = ["provider"]
[[operations]]
kind = "shell"
run = 'test "${PATH%%:*}" = "$HOME/tools"; touch "$HOME/used"'
"#,
    );
    f.success(&["apply", "consumer"]);
    assert!(f.home.join("used").exists());
    f.module(
        "provider",
        "[env]\npath_export = \"printf 'relative\\\\0'\"",
    );
    let out = f.run(&["apply", "consumer"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("env.path_export"));
}

#[test]
fn inherited_bash_startup_and_functions_are_not_used() {
    let f = Fixture::new();
    let startup = f.root.path().join("startup");
    std::fs::write(&startup, "touch \"$HOME/startup-ran\"\n").unwrap();
    f.module(
        "a",
        r#"[[operations]]
kind = "shell"
run = 'test -z "${BASH_ENV-}"; ! declare -F inherited_function; touch "$HOME/clean"'
"#,
    );
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_moi"))
        .args([
            "--source",
            url::Url::from_directory_path(f.root.path())
                .unwrap()
                .as_str(),
            "-e",
            "test",
            "apply",
        ])
        .env("HOME", &f.home)
        .env("BASH_ENV", startup)
        .env("BASH_FUNC_inherited_function%%", "() { return 0; }")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(f.home.join("clean").exists());
    assert!(!f.home.join("startup-ran").exists());
}

#[test]
fn file_and_block_sources_are_frozen_before_shell_changes() {
    let f = Fixture::new();
    let m = f.module(
        "a",
        r#"[[operations]]
kind = "shell"
run = "rm source"
[[operations]]
kind = "file"
src = "source"
dst = "~/result"
"#,
    );
    std::fs::write(m.join("source"), "original").unwrap();
    f.success(&["apply"]);
    assert_eq!(
        std::fs::read_to_string(f.home.join("result")).unwrap(),
        "original"
    );
}

#[test]
fn module_selection_has_lexicographic_ready_order_and_no_reverse_selection() {
    let f = Fixture::new();
    for (name, dependencies) in [("a", "[\"z\"]"), ("b", "[]"), ("z", "[]")] {
        f.module(name,&format!("depends_on = {dependencies}\n[[operations]]\nkind = \"shell\"\nrun = 'printf {name} >> \"$HOME/order\"'"));
    }
    f.success(&["apply"]);
    assert_eq!(
        std::fs::read_to_string(f.home.join("order")).unwrap(),
        "bza"
    );
    std::fs::remove_file(f.home.join("order")).unwrap();
    f.success(&["apply", "z"]);
    assert_eq!(std::fs::read_to_string(f.home.join("order")).unwrap(), "z");
}

#[test]
fn missing_install_environment_cannot_fall_back_to_config_or_env() {
    let f = Fixture::new();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_moi"))
        .args(["install-command"])
        .env("HOME", &f.home)
        .env("MOI_ENVIRONMENT", "test")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("environment"));
}

#[test]
fn absent_inputs_on_other_architectures_are_not_required_and_exclusive_targets_can_coexist()
 {
    let f = Fixture::new();
    let other = if std::env::consts::ARCH == "x86_64" {
        "aarch64"
    } else {
        "x86_64"
    };
    f.module(
        "a",
        &format!(
            r#"[[operations]]
kind = "file"
src = "missing"
dst = "~/unused"
when = {{ arch = [{other:?}] }}
[[operations]]
kind = "directory"
path = "~/unused"
when = {{ arch = [{:?}] }}
"#,
            std::env::consts::ARCH
        ),
    );
    let out = f.success(&["apply"]);
    assert!(f.home.join("unused").is_dir());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not-applicable=1"));
}

#[test]
fn planned_symlink_parents_conflict_even_when_the_link_does_not_exist_yet() {
    let f = Fixture::new();
    f.module(
        "a",
        r#"[[operations]]
kind = "symlink"
target = "~/elsewhere"
dst = "~/link"
[[operations]]
kind = "directory"
path = "~/link/child"
"#,
    );
    assert!(!f.run(&["apply"]).status.success());
    assert!(!f.home.join("link").exists());
}

#[test]
fn different_block_markers_coexist_and_reapply_without_changes() {
    let f = Fixture::new();
    let m = f.module(
        "a",
        r#"[[operations]]
kind = "block"
src = "one"
dst = "~/config"
marker = "one"
[[operations]]
kind = "block"
src = "two"
dst = "~/config"
marker = "two"
"#,
    );
    std::fs::write(m.join("one"), "first").unwrap();
    std::fs::write(m.join("two"), "second").unwrap();
    std::fs::write(f.home.join("config"), "unmanaged\n").unwrap();
    f.success(&["apply"]);
    let text = std::fs::read_to_string(f.home.join("config")).unwrap();
    assert!(text.starts_with("unmanaged\n"));
    assert!(text.contains("first") && text.contains("second"));
    let out = f.success(&["apply"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("unchanged=2"));
}

#[test]
fn symlink_manages_its_own_target_and_rejects_regular_files() {
    let f = Fixture::new();
    f.module(
        "a",
        r#"[[operations]]
kind = "symlink"
target = "~/missing"
dst = "~/link"
"#,
    );
    f.success(&["apply"]);
    assert_eq!(
        std::fs::read_link(f.home.join("link")).unwrap(),
        f.home.join("missing")
    );
    let out = f.success(&["apply"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("unchanged=1"));
    std::fs::remove_file(f.home.join("link")).unwrap();
    std::fs::write(f.home.join("link"), "keep").unwrap();
    assert!(!f.run(&["apply"]).status.success());
    assert_eq!(
        std::fs::read_to_string(f.home.join("link")).unwrap(),
        "keep"
    );
}

#[test]
fn config_precedence_is_cli_then_environment_then_file_and_reads_do_not_change_it() {
    let f = Fixture::new();
    f.module("a", "");
    let config = f.home.join(".config/moi/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let source = url::Url::from_directory_path(f.root.path())
        .unwrap()
        .to_string();
    let content =
        format!("default_environment = 'test'\ndefault_source = {source:?}\n");
    std::fs::write(&config, &content).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_moi"))
        .arg("plan")
        .env("HOME", &f.home)
        .env_remove("MOI_ENVIRONMENT")
        .env_remove("MOI_SOURCE")
        .env_remove("MOI_FOLDER_NAME")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_moi"))
        .arg("plan")
        .env("HOME", &f.home)
        .env("MOI_ENVIRONMENT", "missing")
        .env_remove("MOI_SOURCE")
        .env_remove("MOI_FOLDER_NAME")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_moi"))
        .args(["-e", "test", "plan"])
        .env("HOME", &f.home)
        .env("MOI_ENVIRONMENT", "missing")
        .env_remove("MOI_SOURCE")
        .env_remove("MOI_FOLDER_NAME")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(config).unwrap(), content);
}

#[test]
fn install_command_quotes_arguments_and_expands_local_home_source() {
    let f = Fixture::new();
    let out = f.success(&[
        "install-command",
        "--install-source",
        "~/source with spaces",
        "plan",
        "--show-note",
        "module; touch /tmp/never",
    ]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains(&format!(
        "'{}/source with spaces/install.sh'",
        f.home.display()
    )));
    assert!(stdout.contains("'module; touch /tmp/never'"));
    assert!(stdout.contains("'plan' '--show-note'"));
    assert_eq!(std::fs::read_dir(&f.home).unwrap().count(), 0);
}

#[test]
fn shared_static_inputs_are_allowed_within_the_checkout_and_rejected_outside_it() {
    let f = Fixture::new();
    let shared = f.root.path().join("shared");
    std::fs::write(&shared, "shared").unwrap();
    let m = f.module(
        "a",
        r#"[[operations]]
kind = "file"
src = "source"
dst = "~/result"
"#,
    );
    std::os::unix::fs::symlink(&shared, m.join("source")).unwrap();
    f.success(&["apply"]);
    assert_eq!(
        std::fs::read_to_string(f.home.join("result")).unwrap(),
        "shared"
    );
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::remove_file(m.join("source")).unwrap();
    std::os::unix::fs::symlink(outside.path(), m.join("source")).unwrap();
    assert!(!f.run(&["plan"]).status.success());
}
