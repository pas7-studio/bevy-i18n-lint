// File: src/lib.rs
//! bevy-i18n-lint core library.
//!
//! ## Quick example
//! ```no_run
//! use bevy_i18n_lint::{run, CliOptions};
//! use std::path::PathBuf;
//!
//! let _ = run(CliOptions {
//!     dir: PathBuf::from("assets/i18n"),
//!     base: "en".to_string(),
//!     strict: false,
//!     format: "text".to_string(),
//!     fail_on_extra: false,
//!     fail_on_placeholder: false,
//! });
//! ```

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct CliOptions {
    pub dir: PathBuf,
    pub base: String,
    pub strict: bool,
    pub format: String,
    pub fail_on_extra: bool,
    pub fail_on_placeholder: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub kind: String,
    pub lang: String,
    pub key: String,
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub base: String,
    pub langs: Vec<String>,
    pub missing: Vec<Finding>,
    pub extra: Vec<Finding>,
    pub placeholder_mismatch: Vec<Finding>,
    pub totals: Totals,
}

#[derive(Debug, Clone, Serialize)]
pub struct Totals {
    pub missing: usize,
    pub extra: usize,
    pub placeholder_mismatch: usize,
}

pub fn run(opts: CliOptions) -> Result<i32> {
    let files = discover_locale_files(&opts.dir)?;
    if files.is_empty() {
        return Err(anyhow!("no locale files found in {}", opts.dir.display()));
    }

    let mut by_lang: BTreeMap<String, PathBuf> = BTreeMap::new();
    for f in files {
        if let Some(lang) = lang_from_filename(&f) {
            by_lang.insert(lang, f);
        }
    }

    if !by_lang.contains_key(&opts.base) {
        return Err(anyhow!(
            "base language '{}' not found in {}",
            opts.base,
            opts.dir.display()
        ));
    }

    let base_file = by_lang.get(&opts.base).unwrap().clone();
    let base_map = load_kv(&base_file)?;
    let base_keys: BTreeSet<String> = base_map.keys().cloned().collect();

    let placeholder_re = Regex::new(r"\{([A-Za-z0-9_]+)\}").unwrap();

    let mut missing = Vec::new();
    let mut extra = Vec::new();
    let mut placeholder_mismatch = Vec::new();

    for (lang, file) in by_lang.iter() {
        if *lang == opts.base {
            continue;
        }

        let map = load_kv(file)?;
        let keys: BTreeSet<String> = map.keys().cloned().collect();

        for k in base_keys.iter() {
            if !keys.contains(k) {
                missing.push(Finding {
                    kind: "missing_key".to_string(),
                    lang: lang.clone(),
                    key: k.clone(),
                    file: file.display().to_string(),
                    message: format!("key '{}' is missing (base: {})", k, opts.base),
                });
            } else {
                let base_v = base_map.get(k).cloned().unwrap_or_default();
                let v = map.get(k).cloned().unwrap_or_default();

                let bp = extract_placeholders(&placeholder_re, &base_v);
                let tp = extract_placeholders(&placeholder_re, &v);
                if bp != tp {
                    placeholder_mismatch.push(Finding {
                        kind: "placeholder_mismatch".to_string(),
                        lang: lang.clone(),
                        key: k.clone(),
                        file: file.display().to_string(),
                        message: format!(
                            "placeholders mismatch for key '{}': base={:?}, {}={:?}",
                            k, bp, lang, tp
                        ),
                    });
                }
            }
        }

        for k in keys.iter() {
            if !base_keys.contains(k) {
                extra.push(Finding {
                    kind: "extra_key".to_string(),
                    lang: lang.clone(),
                    key: k.clone(),
                    file: file.display().to_string(),
                    message: format!(
                        "key '{}' exists in {}, but not in base {}",
                        k, lang, opts.base
                    ),
                });
            }
        }
    }

    let langs = by_lang.keys().cloned().collect::<Vec<_>>();

    let report = Report {
        base: opts.base.clone(),
        langs,
        missing: missing.clone(),
        extra: extra.clone(),
        placeholder_mismatch: placeholder_mismatch.clone(),
        totals: Totals {
            missing: missing.len(),
            extra: extra.len(),
            placeholder_mismatch: placeholder_mismatch.len(),
        },
    };

    let mut exit_code = 0;

    if opts.strict && (!missing.is_empty() || !extra.is_empty() || !placeholder_mismatch.is_empty())
    {
        exit_code = 1;
    } else {
        if !missing.is_empty() {
            exit_code = 1;
        }
        if opts.fail_on_extra && !extra.is_empty() {
            exit_code = 1;
        }
        if opts.fail_on_placeholder && !placeholder_mismatch.is_empty() {
            exit_code = 1;
        }
    }

    match opts.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "github" => {
            emit_github_annotations(&missing);
            emit_github_annotations(&extra);
            emit_github_annotations(&placeholder_mismatch);
            println!(
                "bevy-i18n-lint: missing={}, extra={}, placeholder_mismatch={}",
                report.totals.missing, report.totals.extra, report.totals.placeholder_mismatch
            );
        }
        _ => {
            emit_text(&report);
        }
    }

    Ok(exit_code)
}

fn discover_locale_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(dir).follow_links(true) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "json" || ext == "ron" {
            out.push(p.to_path_buf());
        }
    }
    Ok(out)
}

fn lang_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    if stem.is_empty() {
        None
    } else {
        Some(stem)
    }
}

fn load_kv(path: &Path) -> Result<BTreeMap<String, String>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let data = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;

    match ext {
        "json" => {
            let v: serde_json::Value = serde_json::from_str(&data)
                .with_context(|| format!("parse json {}", path.display()))?;
            let mut out = BTreeMap::new();
            flatten_json_value("", &v, &mut out);
            Ok(out)
        }
        "ron" => {
            let v: ron::Value =
                ron::from_str(&data).with_context(|| format!("parse ron {}", path.display()))?;
            let mut out = BTreeMap::new();
            flatten_ron_value("", &v, &mut out);
            Ok(out)
        }
        _ => Err(anyhow!("unsupported file extension: {}", path.display())),
    }
}

fn flatten_json_value(prefix: &str, v: &serde_json::Value, out: &mut BTreeMap<String, String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, vv) in map.iter() {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json_value(&p, vv, out);
            }
        }
        serde_json::Value::String(s) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), s.clone());
            }
        }
        serde_json::Value::Number(n) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), n.to_string());
            }
        }
        serde_json::Value::Bool(b) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), b.to_string());
            }
        }
        serde_json::Value::Null => {}
        serde_json::Value::Array(_) => {}
    }
}

fn flatten_ron_value(prefix: &str, v: &ron::Value, out: &mut BTreeMap<String, String>) {
    match v {
        ron::Value::Map(map) => {
            for (k, vv) in map.iter() {
                let kk = match k {
                    ron::Value::String(s) => s.clone(),
                    ron::Value::Number(n) => format!("{:?}", n),
                    ron::Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                let p = if prefix.is_empty() {
                    kk
                } else {
                    format!("{}.{}", prefix, kk)
                };
                flatten_ron_value(&p, vv, out);
            }
        }
        ron::Value::String(s) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), s.clone());
            }
        }
        ron::Value::Number(n) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), format!("{:?}", n));
            }
        }
        ron::Value::Bool(b) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), b.to_string());
            }
        }
        ron::Value::Option(o) => {
            if let Some(inner) = o.as_ref() {
                flatten_ron_value(prefix, inner, out);
            }
        }
        ron::Value::Seq(_) => {}
        ron::Value::Char(c) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), c.to_string());
            }
        }
        ron::Value::Unit => {}
    }
}

fn extract_placeholders(re: &Regex, s: &str) -> BTreeSet<String> {
    re.captures_iter(s)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn emit_text(report: &Report) {
    println!(
        "bevy-i18n-lint: base={}, langs={}",
        report.base,
        report.langs.join(", ")
    );

    if report.missing.is_empty()
        && report.extra.is_empty()
        && report.placeholder_mismatch.is_empty()
    {
        println!("ok: no issues found");
        return;
    }

    if !report.missing.is_empty() {
        println!("\nmissing keys: {}", report.missing.len());
        for f in report.missing.iter() {
            println!("  [{}] {} -> {}", f.lang, f.key, f.file);
        }
    }

    if !report.extra.is_empty() {
        println!("\nextra keys: {}", report.extra.len());
        for f in report.extra.iter() {
            println!("  [{}] {} -> {}", f.lang, f.key, f.file);
        }
    }

    if !report.placeholder_mismatch.is_empty() {
        println!(
            "\nplaceholder mismatches: {}",
            report.placeholder_mismatch.len()
        );
        for f in report.placeholder_mismatch.iter() {
            println!("  [{}] {} -> {}", f.lang, f.key, f.file);
        }
    }

    println!(
        "\nsummary: missing={}, extra={}, placeholder_mismatch={}",
        report.totals.missing, report.totals.extra, report.totals.placeholder_mismatch
    );
}

fn emit_github_annotations(items: &[Finding]) {
    for f in items.iter() {
        let msg = f.message.replace(['\n', '\r'], " ");
        println!("::error file={},line=1,col=1::{}", f.file, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_missing_key_and_placeholder_mismatch_json_nested() {
        let dir = TempDir::new().unwrap();
        let base_json = r#"{
            "ui": {
                "buttons": {
                    "save": "Save {item}",
                    "cancel": "Cancel"
                },
                "messages": {
                    "welcome": "Welcome {user}"
                }
            }
        }"#;
        let other_json = r#"{
            "ui": {
                "buttons": {
                    "save": "Зберегти"
                },
                "messages": {
                    "welcome": "Ласкаво просимо {username}"
                }
            }
        }"#;

        fs::write(dir.path().join("en.json"), base_json).unwrap();
        fs::write(dir.path().join("uk.json"), other_json).unwrap();

        let opts = CliOptions {
            dir: dir.path().to_path_buf(),
            base: "en".to_string(),
            strict: false,
            format: "json".to_string(),
            fail_on_extra: false,
            fail_on_placeholder: false,
        };

        let result = run(opts);
        assert!(result.is_ok());
    }

    #[test]
    fn detects_extra_keys_and_ron_flattening() {
        let dir = TempDir::new().unwrap();
        let base_ron = r#"(
            ui: {
                buttons: {
                    save: "Save",
                    cancel: "Cancel",
                },
            },
        )"#;
        let other_ron = r#"(
            ui: {
                buttons: {
                    save: "Зберегти",
                    cancel: "Скасувати",
                    delete: "Видалити",
                },
            },
        )"#;

        fs::write(dir.path().join("en.ron"), base_ron).unwrap();
        fs::write(dir.path().join("uk.ron"), other_ron).unwrap();

        let opts = CliOptions {
            dir: dir.path().to_path_buf(),
            base: "en".to_string(),
            strict: false,
            format: "json".to_string(),
            fail_on_extra: false,
            fail_on_placeholder: false,
        };

        let result = run(opts);
        assert!(result.is_ok());
    }

    #[test]
    fn ok_when_all_languages_match() {
        let dir = TempDir::new().unwrap();
        let base_json = r#"{
            "save": "Save {item}",
            "cancel": "Cancel"
        }"#;
        let other_json = r#"{
            "save": "Зберегти {item}",
            "cancel": "Скасувати"
        }"#;

        fs::write(dir.path().join("en.json"), base_json).unwrap();
        fs::write(dir.path().join("uk.json"), other_json).unwrap();

        let opts = CliOptions {
            dir: dir.path().to_path_buf(),
            base: "en".to_string(),
            strict: true,
            format: "json".to_string(),
            fail_on_extra: false,
            fail_on_placeholder: false,
        };

        let result = run(opts);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
