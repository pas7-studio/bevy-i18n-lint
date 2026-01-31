use std::fs;
use std::process::Command;

fn bin() -> Command {
    let exe = env!("CARGO_BIN_EXE_bevy-i18n-lint");
    Command::new(exe)
}

fn write(dir: &std::path::Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).unwrap();
}

#[test]
fn cli_ok_when_all_match() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(
        root,
        "en.json",
        r#"{"ui":{"play":"Play","hello":"Hello {name}"}}"#,
    );
    write(
        root,
        "uk.json",
        r#"{"ui":{"play":"Грати","hello":"Привіт {name}"}}"#,
    );

    let out = bin()
        .arg("--dir")
        .arg(root)
        .arg("--base")
        .arg("en")
        .output()
        .unwrap();

    assert!(out.status.success());
}

#[test]
fn cli_fails_on_missing_key() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(
        root,
        "en.json",
        r#"{"ui":{"play":"Play","hello":"Hello {name}"}}"#,
    );
    write(root, "uk.json", r#"{"ui":{"hello":"Привіт {name}"}}"#);

    let out = bin()
        .arg("--dir")
        .arg(root)
        .arg("--base")
        .arg("en")
        .output()
        .unwrap();

    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("missing keys"));
}

#[test]
fn cli_json_format_is_valid_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(
        root,
        "en.json",
        r#"{"ui":{"play":"Play","hello":"Hello {name}"}}"#,
    );
    write(root, "uk.json", r#"{"ui":{"hello":"Привіт {name}"}}"#);

    let out = bin()
        .arg("--dir")
        .arg(root)
        .arg("--base")
        .arg("en")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(!out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v.get("totals").is_some());
    assert!(v.get("missing").is_some());
}

#[test]
fn cli_github_format_emits_annotations() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(root, "en.json", r#"{"ui":{"play":"Play"}}"#);
    write(root, "uk.json", r#"{"ui":{}}"#);

    let out = bin()
        .arg("--dir")
        .arg(root)
        .arg("--base")
        .arg("en")
        .arg("--format")
        .arg("github")
        .output()
        .unwrap();

    assert!(!out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("::error file="));
}
