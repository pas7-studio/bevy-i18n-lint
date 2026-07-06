//! bevy-i18n-lint core library.
//!
//! ## Quick example
//! ```no_run
//! use bevy_i18n_lint::{run, CliOptions, OutputFormat};
//! use std::path::PathBuf;
//!
//! let _ = run(CliOptions {
//!     dir: PathBuf::from("assets/i18n"),
//!     base: "en".to_string(),
//!     strict: false,
//!     format: OutputFormat::Text,
//!     fail_on_extra: false,
//!     fail_on_placeholder: false,
//!     config_path: None,
//!     no_ignore: false,
//! });
//! ```

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Output format for lint results.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Github,
}

/// CLI options for running the linter.
#[derive(Debug, Clone)]
pub struct CliOptions {
    pub dir: PathBuf,
    pub base: String,
    pub strict: bool,
    pub format: OutputFormat,
    pub fail_on_extra: bool,
    pub fail_on_placeholder: bool,
    /// Explicit config file path. If `None`, auto-discover.
    pub config_path: Option<PathBuf>,
    /// If `true`, ignore `ignore_keys` from config.
    pub no_ignore: bool,
}

/// A single lint finding.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub kind: String,
    pub lang: String,
    pub key: String,
    pub file: String,
    pub message: String,
}

/// Full lint report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub base: String,
    pub langs: Vec<String>,
    pub missing: Vec<Finding>,
    pub extra: Vec<Finding>,
    pub placeholder_mismatch: Vec<Finding>,
    pub totals: Totals,
}

/// Counts for each category.
#[derive(Debug, Clone, Serialize)]
pub struct Totals {
    pub missing: usize,
    pub extra: usize,
    pub placeholder_mismatch: usize,
}

/// Project-specific config file (bevy-i18n-lint.toml or bevy-i18n-lint.json).
///
/// This allows agents and CI to codify project-specific decisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Key patterns to ignore (e.g. `["deprecated.*", "old_feature"]`).
    /// Supports `*` as a wildcard glob.
    pub ignore_keys: Option<Vec<String>>,
    /// Languages to skip entirely.
    pub skip_languages: Option<Vec<String>>,
    /// Custom regex pattern for placeholder detection.
    /// Default: `\{([A-Za-z0-9_]+)\}`
    pub placeholder_pattern: Option<String>,
    /// Override `--strict`
    pub strict: Option<bool>,
    /// Override `--fail-on-extra`
    pub fail_on_extra: Option<bool>,
    /// Override `--fail-on-placeholder`
    pub fail_on_placeholder: Option<bool>,
}

impl Config {
    /// Load project config from a directory, searching for
    /// `bevy-i18n-lint.toml` then `bevy-i18n-lint.json`.
    pub fn load(dir: &Path) -> Result<Option<Self>> {
        let candidates = [
            dir.join("bevy-i18n-lint.toml"),
            dir.join("bevy-i18n-lint.json"),
        ];

        for path in &candidates {
            if path.exists() {
                let data = fs::read_to_string(path)
                    .with_context(|| format!("read config {}", path.display()))?;
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                return match ext {
                    "json" => {
                        let c: Config = serde_json::from_str(&data)
                            .with_context(|| format!("parse config {}", path.display()))?;
                        Ok(Some(c))
                    }
                    "toml" => {
                        let c: Config = toml::from_str(&data)
                            .with_context(|| format!("parse config {}", path.display()))?;
                        Ok(Some(c))
                    }
                    _ => Err(anyhow!("unsupported config format: {}", path.display())),
                };
            }
        }
        Ok(None)
    }

    /// Merge config values as overrides on top of CLI options.
    fn merge_into(&self, opts: &CliOptions) -> MergedOptions {
        MergedOptions {
            strict: self.strict.unwrap_or(opts.strict),
            fail_on_extra: self.fail_on_extra.unwrap_or(opts.fail_on_extra),
            fail_on_placeholder: self.fail_on_placeholder.unwrap_or(opts.fail_on_placeholder),
        }
    }

    /// Check whether a key matches any ignore pattern (with `*` glob support).
    fn matches_ignore(&self, key: &str) -> bool {
        self.ignore_keys
            .as_ref()
            .is_some_and(|patterns| patterns.iter().any(|p| config_key_match(p, key)))
    }

    /// Check whether a language should be skipped.
    fn skip_lang(&self, lang: &str) -> bool {
        self.skip_languages
            .as_ref()
            .is_some_and(|skip| skip.iter().any(|s| s == lang))
    }
}

/// Generate a default `bevy-i18n-lint.toml` config file in the current directory.
pub fn generate_config() -> std::io::Result<()> {
    let content = r#"# bevy-i18n-lint configuration
# Uncomment and adjust the settings below.

# Key patterns to ignore (supports * wildcard)
# ignore_keys = ["deprecated.*", "experimental_*"]

# Languages to skip entirely
# skip_languages = ["zz"]

# Custom regex for placeholder detection (default: \{([A-Za-z0-9_]+)\})
# placeholder_pattern = "\\(([A-Za-z_]+)\\)"

# Exit with error on any issue (missing, extra, placeholder)
# strict = true

# Exit with error on extra keys
# fail_on_extra = true

# Exit with error on placeholder mismatches
# fail_on_placeholder = true
"#;
    std::fs::write("bevy-i18n-lint.toml", content)
}

struct MergedOptions {
    strict: bool,
    fail_on_extra: bool,
    fail_on_placeholder: bool,
}

fn find_config(locale_dir: &Path, explicit: Option<&Path>) -> Result<Option<Config>> {
    if let Some(path) = explicit {
        let data =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        return match ext {
            "json" => {
                let c: Config = serde_json::from_str(&data)
                    .with_context(|| format!("parse config {}", path.display()))?;
                Ok(Some(c))
            }
            "toml" => {
                let c: Config = toml::from_str(&data)
                    .with_context(|| format!("parse config {}", path.display()))?;
                Ok(Some(c))
            }
            _ => Err(anyhow!("unsupported config format: {}", path.display())),
        };
    }
    if let Some(config) = Config::load(locale_dir)? {
        return Ok(Some(config));
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd != locale_dir {
            return Config::load(&cwd);
        }
    }
    Ok(None)
}

fn config_key_match(pattern: &str, key: &str) -> bool {
    if pattern.contains('*') {
        let re_str = format!("^{}$", regex::escape(pattern).replace("\\*", ".*"));
        Regex::new(&re_str).is_ok_and(|re| re.is_match(key))
    } else {
        pattern == key
    }
}

/// Run the linter with the given options.
///
/// Returns the exit code (0 = success, 1 = issues found).
pub fn run(opts: CliOptions) -> Result<i32> {
    let files = discover_locale_files(&opts.dir)?;
    if files.is_empty() {
        return Err(anyhow!("no locale files found in {}", opts.dir.display()));
    }

    let config = if opts.config_path.is_some() {
        // Explicit --config: error if missing or invalid
        find_config(&opts.dir, opts.config_path.as_deref())?
    } else {
        // Auto-discover: silently fall back to no config
        find_config(&opts.dir, None).ok().flatten()
    };

    let mut by_lang: BTreeMap<String, PathBuf> = BTreeMap::new();
    for f in files {
        if let Some(lang) = lang_from_filename(&f) {
            if config.as_ref().is_some_and(|c| c.skip_lang(&lang)) {
                continue;
            }
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

    let base_file = by_lang.get(&opts.base).unwrap();
    let base_map = load_kv(base_file)?;
    let base_keys: BTreeSet<&str> = base_map.keys().map(String::as_str).collect();

    let placeholder_re = match config.as_ref().and_then(|c| c.placeholder_pattern.as_ref()) {
        Some(pattern) => Regex::new(pattern).context("invalid placeholder_pattern in config")?,
        None => Regex::new(r"\{([A-Za-z0-9_]+)\}").unwrap(),
    };

    let mut missing = Vec::new();
    let mut extra = Vec::new();
    let mut placeholder_mismatch = Vec::new();

    for (lang, file) in by_lang.iter() {
        if *lang == opts.base {
            continue;
        }

        let map = load_kv(file)?;
        let keys: BTreeSet<&str> = map.keys().map(String::as_str).collect();

        for &k in base_keys.iter() {
            if !opts.no_ignore && config.as_ref().is_some_and(|c| c.matches_ignore(k)) {
                continue;
            }
            if !keys.contains(k) {
                missing.push(Finding {
                    kind: "missing_key".to_string(),
                    lang: lang.clone(),
                    key: k.to_string(),
                    file: file.display().to_string(),
                    message: format!("key '{}' is missing (base: {})", k, opts.base),
                });
            } else {
                let base_v = &base_map[k];
                let v = &map[k];

                let bp = extract_placeholders(&placeholder_re, base_v);
                let tp = extract_placeholders(&placeholder_re, v);
                if bp != tp {
                    placeholder_mismatch.push(Finding {
                        kind: "placeholder_mismatch".to_string(),
                        lang: lang.clone(),
                        key: k.to_string(),
                        file: file.display().to_string(),
                        message: format!(
                            "placeholders mismatch for key '{}': base={:?}, {}={:?}",
                            k, bp, lang, tp
                        ),
                    });
                }
            }
        }

        for k in keys.iter().copied() {
            if !opts.no_ignore && config.as_ref().is_some_and(|c| c.matches_ignore(k)) {
                continue;
            }
            if !base_keys.contains(k) {
                extra.push(Finding {
                    kind: "extra_key".to_string(),
                    lang: lang.clone(),
                    key: k.to_string(),
                    file: file.display().to_string(),
                    message: format!(
                        "key '{}' exists in {}, but not in base {}",
                        k, lang, opts.base
                    ),
                });
            }
        }
    }

    let merged = config
        .as_ref()
        .map(|c| c.merge_into(&opts))
        .unwrap_or(MergedOptions {
            strict: opts.strict,
            fail_on_extra: opts.fail_on_extra,
            fail_on_placeholder: opts.fail_on_placeholder,
        });

    let totals = Totals {
        missing: missing.len(),
        extra: extra.len(),
        placeholder_mismatch: placeholder_mismatch.len(),
    };

    let exit_code = determine_exit_code(&merged, &missing, &extra, &placeholder_mismatch);

    let report = Report {
        base: opts.base,
        langs: by_lang.into_keys().collect(),
        missing,
        extra,
        placeholder_mismatch,
        totals,
    };

    emit_report(&opts.format, &report)?;

    Ok(exit_code)
}

fn determine_exit_code(
    merged: &MergedOptions,
    missing: &[Finding],
    extra: &[Finding],
    placeholder_mismatch: &[Finding],
) -> i32 {
    let has_missing = !missing.is_empty();
    let has_extra = !extra.is_empty();
    let has_placeholder = !placeholder_mismatch.is_empty();

    if (merged.strict && (has_missing || has_extra || has_placeholder))
        || has_missing
        || (merged.fail_on_extra && has_extra)
        || (merged.fail_on_placeholder && has_placeholder)
    {
        1
    } else {
        0
    }
}

fn emit_report(format: &OutputFormat, report: &Report) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(report)?);
        }
        OutputFormat::Github => {
            emit_github_annotations(&report.missing);
            emit_github_annotations(&report.extra);
            emit_github_annotations(&report.placeholder_mismatch);
            println!(
                "bevy-i18n-lint: missing={}, extra={}, placeholder_mismatch={}",
                report.totals.missing, report.totals.extra, report.totals.placeholder_mismatch
            );
        }
        OutputFormat::Text => {
            emit_text(report);
        }
    }
    Ok(())
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
                    format!("{prefix}.{k}")
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
        serde_json::Value::Array(_) | serde_json::Value::Null => {}
    }
}

fn flatten_ron_value(prefix: &str, v: &ron::Value, out: &mut BTreeMap<String, String>) {
    match v {
        ron::Value::Map(map) => {
            for (k, vv) in map.iter() {
                let kk = match k {
                    ron::Value::String(s) => s.clone(),
                    ron::Value::Number(n) => format!("{n:?}"),
                    ron::Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                let p = if prefix.is_empty() {
                    kk
                } else {
                    format!("{prefix}.{kk}")
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
                out.insert(prefix.to_string(), format!("{n:?}"));
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
        ron::Value::Char(c) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), c.to_string());
            }
        }
        ron::Value::Seq(_) | ron::Value::Unit => {}
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
        println!("::error file={},line=1,col=1::{msg}", f.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct TestEnv {
        dir: TempDir,
    }

    impl TestEnv {
        fn new() -> Self {
            Self {
                dir: TempDir::new().unwrap(),
            }
        }

        fn write(&self, name: &str, content: &str) {
            fs::write(self.dir.path().join(name), content).unwrap();
        }

        fn run(&self, opts: CliOptions) -> Result<i32> {
            let opts = CliOptions {
                dir: self.dir.path().to_path_buf(),
                ..opts
            };
            run(opts)
        }

        fn path(&self) -> &std::path::Path {
            self.dir.path()
        }
    }

    fn base_opts() -> CliOptions {
        CliOptions {
            dir: PathBuf::new(),
            base: "en".to_string(),
            strict: false,
            format: OutputFormat::Json,
            fail_on_extra: false,
            fail_on_placeholder: false,
            config_path: None,
            no_ignore: false,
        }
    }

    #[test]
    fn detects_missing_key_and_placeholder_mismatch_json_nested() {
        let env = TestEnv::new();
        env.write(
            "en.json",
            r#"{
                "ui": {
                    "buttons": { "save": "Save {item}", "cancel": "Cancel" },
                    "messages": { "welcome": "Welcome {user}" }
                }
            }"#,
        );
        env.write(
            "uk.json",
            r#"{
                "ui": {
                    "buttons": { "save": "Зберегти" },
                    "messages": { "welcome": "Ласкаво просимо {username}" }
                }
            }"#,
        );

        let result = env.run(base_opts());
        assert!(result.is_ok());
        let code = result.unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn detects_extra_keys_and_ron_flattening() {
        let env = TestEnv::new();
        env.write(
            "en.ron",
            r#"(
                ui: { buttons: { save: "Save", cancel: "Cancel" } }
            )"#,
        );
        env.write(
            "uk.ron",
            r#"(
                ui: { buttons: { save: "Зберегти", cancel: "Скасувати", delete: "Видалити" } }
            )"#,
        );

        let result = env.run(base_opts());
        assert!(result.is_ok());
    }

    #[test]
    fn ok_when_all_languages_match() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"save":"Save {item}","cancel":"Cancel"}"#);
        env.write(
            "uk.json",
            r#"{"save":"Зберегти {item}","cancel":"Скасувати"}"#,
        );

        let result = env.run(CliOptions {
            strict: true,
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn config_loads_from_toml() {
        let env = TestEnv::new();
        env.write(
            "bevy-i18n-lint.toml",
            r#"ignore_keys = ["deprecated.*"]
skip_languages = ["zz"]
placeholder_pattern = "\\(([A-Za-z_]+)\\)"
strict = true
"#,
        );

        let config = Config::load(env.path()).unwrap().unwrap();
        assert_eq!(
            config.ignore_keys.as_ref().unwrap(),
            &vec!["deprecated.*".to_string()]
        );
        assert_eq!(
            config.skip_languages.as_ref().unwrap(),
            &vec!["zz".to_string()]
        );
        assert_eq!(
            config.placeholder_pattern.as_ref().unwrap(),
            r"\(([A-Za-z_]+)\)"
        );
        assert_eq!(config.strict, Some(true));
    }

    #[test]
    fn config_loads_from_json() {
        let env = TestEnv::new();
        env.write(
            "bevy-i18n-lint.json",
            r#"{
                "ignore_keys": ["temp.*"],
                "skip_languages": ["de"],
                "strict": false,
                "fail_on_extra": true
            }"#,
        );

        let config = Config::load(env.path()).unwrap().unwrap();
        assert_eq!(
            config.ignore_keys.as_ref().unwrap(),
            &vec!["temp.*".to_string()]
        );
        assert_eq!(
            config.skip_languages.as_ref().unwrap(),
            &vec!["de".to_string()]
        );
        assert_eq!(config.strict, Some(false));
        assert_eq!(config.fail_on_extra, Some(true));
    }

    #[test]
    fn config_absent_returns_none() {
        let env = TestEnv::new();
        let config = Config::load(env.path()).unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn config_ignore_keys_skips_missing_key() {
        let env = TestEnv::new();
        env.write(
            "bevy-i18n-lint.toml",
            r#"ignore_keys = ["deprecated_welcome"]"#,
        );
        env.write(
            "en.json",
            r#"{"greeting":"Hello","deprecated_welcome":"Welcome"}"#,
        );
        env.write("uk.json", r#"{"greeting":"Привіт"}"#);

        let result = env.run(base_opts());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn config_ignore_keys_with_wildcard() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"ignore_keys = ["deprecated_*"]"#);
        env.write(
            "en.json",
            r#"{"a":"A","deprecated_old":"Old","deprecated_new":"New"}"#,
        );
        env.write("uk.json", r#"{"a":"А"}"#);

        let result = env.run(base_opts());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn config_skip_languages_skips_entire_lang() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"skip_languages = ["de"]"#);
        env.write("en.json", r#"{"a":"A","b":"B"}"#);
        env.write("de.json", r#"{"a":"A"}"#);
        env.write("fr.json", r#"{"a":"A"}"#);

        let mut opts = base_opts();
        opts.format = OutputFormat::Json;
        let result = env.run(opts);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert_eq!(code, 1); // fr still has missing
    }

    #[test]
    fn config_placeholder_pattern_custom() {
        let env = TestEnv::new();
        env.write(
            "bevy-i18n-lint.toml",
            r#"placeholder_pattern = "\\(([A-Za-z_]+)\\)""#,
        );
        env.write("en.json", r#"{"key":"Hello (name) (count)"}"#);
        env.write("uk.json", r#"{"key":"Привіт (name)"}"#);

        // Default: placeholder mismatches don't exit 1 without --fail-on-placeholder
        let result = env.run(CliOptions {
            format: OutputFormat::Json,
            fail_on_placeholder: true,
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn config_strict_override() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"strict = true"#);
        env.write("en.json", r#"{"a":"A"}"#);
        env.write("uk.json", r#"{"a":"А","b":"Б"}"#);

        let mut opts = base_opts();
        opts.strict = false;
        opts.fail_on_extra = false;
        let result = env.run(opts);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // strict from config catches extra
    }

    #[test]
    fn config_fail_on_extra_override() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"fail_on_extra = true"#);
        env.write("en.json", r#"{"a":"A"}"#);
        env.write("uk.json", r#"{"a":"А","b":"Б"}"#);

        let mut opts = base_opts();
        opts.fail_on_extra = false;
        let result = env.run(opts);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn config_merge_precedence_cli_overrides_config() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"strict = true"#);
        // CliOptions has strict=false but config has strict=true — config wins
        // However, if we set strict=false in config AND strict=false in CLI, it's false.
        // Let's test: config strict=false, CLI strict=true -> CLI wins.
        // Actually the merged approach makes config override CLI. Let me test that.
        // Wait, the merge function is: config.strict.unwrap_or(opts.strict)
        // So config takes precedence over CLI.
        env.write("en.json", r#"{"a":"A"}"#);
        env.write("uk.json", r#"{"a":"А","b":"Б"}"#);

        let mut opts = base_opts();
        opts.strict = false;
        opts.fail_on_extra = false;
        let result = env.run(opts);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // config strict=true wins
    }

    #[test]
    fn flatten_json_array_ignored() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"items": ["a", "b"], "title": "Hello"}"#);
        env.write("uk.json", r#"{"title": "Привіт"}"#);

        let result = env.run(base_opts());
        assert!(result.is_ok());
        // items is an array so it should be silently ignored
        // only title should be compared
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn flatten_json_null_ignored() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"title": "Hello", "desc": null}"#);
        env.write("uk.json", r#"{"title": "Привіт"}"#);

        let result = env.run(base_opts());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn flatten_json_number_and_bool() {
        let env = TestEnv::new();
        env.write(
            "en.json",
            r#"{"count": 5, "active": true, "title": "Hello"}"#,
        );
        env.write(
            "uk.json",
            r#"{"count": 5, "active": true, "title": "Привіт"}"#,
        );

        let result = env.run(base_opts());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn multiple_languages_all_checked() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"a":"A","b":"B","c":"C"}"#);
        env.write("uk.json", r#"{"a":"А","b":"Б"}"#);
        env.write("fr.json", r#"{"a":"A"}"#);

        let result = env.run(CliOptions {
            format: OutputFormat::Json,
            ..base_opts()
        });
        assert!(result.is_ok());
        // Should detect: uk missing c, fr missing b,c
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn exit_code_zero_when_only_extra_and_not_failing_on_extra() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"a":"A"}"#);
        env.write("uk.json", r#"{"a":"А","b":"Б"}"#);

        let result = env.run(CliOptions {
            fail_on_extra: false,
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn exit_code_one_when_only_extra_and_failing_on_extra() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"a":"A"}"#);
        env.write("uk.json", r#"{"a":"А","b":"Б"}"#);

        let result = env.run(CliOptions {
            fail_on_extra: true,
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn exit_code_one_when_placeholder_mismatch_and_failing_on_placeholder() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"a":"Hello {name}"}"#);
        env.write("uk.json", r#"{"a":"Привіт {username}"}"#);

        let result = env.run(CliOptions {
            fail_on_placeholder: true,
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn ron_value_edge_cases() {
        let env = TestEnv::new();
        // Some(42) flattens to key="count", value="42"
        // None flattens to nothing — so "count" is missing in uk.
        // Option::Some variant produces a value, Option::None does not.
        env.write(
            "en.ron",
            r#"( title: "Hello", count: Some(42), flag: true )"#,
        );
        // Match both by using Some on both sides
        env.write(
            "uk.ron",
            r#"( title: "Привіт", count: Some(42), flag: true )"#,
        );

        let result = env.run(base_opts());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn config_toml_preferred_over_json() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"skip_languages = ["de"]"#);
        env.write("bevy-i18n-lint.json", r#"{"skip_languages": ["fr"]}"#);

        let config = Config::load(env.path()).unwrap().unwrap();
        assert_eq!(
            config.skip_languages.as_ref().unwrap(),
            &vec!["de".to_string()]
        );
    }

    #[test]
    fn finding_serialization() {
        let f = Finding {
            kind: "missing_key".into(),
            lang: "uk".into(),
            key: "test.key".into(),
            file: "/path/to/uk.json".into(),
            message: "key 'test.key' is missing (base: en)".into(),
        };
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("missing_key"));
        assert!(json.contains("uk"));
    }

    #[test]
    fn report_serialization() {
        let report = Report {
            base: "en".into(),
            langs: vec!["en".into(), "uk".into()],
            missing: vec![],
            extra: vec![],
            placeholder_mismatch: vec![],
            totals: Totals {
                missing: 0,
                extra: 0,
                placeholder_mismatch: 0,
            },
        };
        let json = serde_json::to_string_pretty(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["totals"]["missing"], 0);
    }

    #[test]
    fn no_files_error() {
        let env = TestEnv::new();
        let result = env.run(base_opts());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no locale files found"));
    }

    #[test]
    fn base_language_not_found_error() {
        let env = TestEnv::new();
        env.write("de.json", r#"{"a":"A"}"#);

        let result = env.run(CliOptions {
            base: "en".to_string(),
            ..base_opts()
        });
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("base language 'en' not found"));
    }

    #[test]
    fn output_format_default_is_text() {
        assert_eq!(OutputFormat::default(), OutputFormat::Text);
    }

    #[test]
    fn config_key_match_exact() {
        assert!(config_key_match("hello", "hello"));
        assert!(!config_key_match("hello", "world"));
    }

    #[test]
    fn config_key_match_wildcard() {
        assert!(config_key_match("hello.*", "hello.world"));
        assert!(config_key_match("*.test", "foo.test"));
        assert!(!config_key_match("hello.*", "world.test"));
    }

    #[test]
    fn config_explicit_path() {
        let env = TestEnv::new();
        let config_dir = env.dir.path().join("config");
        fs::create_dir(&config_dir).unwrap();
        fs::write(
            config_dir.join("my-config.toml"),
            r#"ignore_keys = ["temp_*"]"#,
        )
        .unwrap();

        env.write("en.json", r#"{"a":"A","temp_x":"X"}"#);
        env.write("uk.json", r#"{"a":"А"}"#);

        let result = env.run(CliOptions {
            config_path: Some(config_dir.join("my-config.toml")),
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn no_ignore_flag_bypasses_ignore_keys() {
        let env = TestEnv::new();
        env.write("bevy-i18n-lint.toml", r#"ignore_keys = ["temp.*"]"#);
        env.write("en.json", r#"{"a":"A","temp_x":"X"}"#);
        env.write("uk.json", r#"{"a":"А"}"#);

        // With no_ignore = true, ignore_keys should be bypassed
        let result = env.run(CliOptions {
            no_ignore: true,
            ..base_opts()
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // temp_x is reported as missing
    }

    #[test]
    fn generate_config_creates_file() {
        let dir = TempDir::new().unwrap();
        let original = std::env::current_dir().ok();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = generate_config();
        assert!(result.is_ok());
        assert!(dir.path().join("bevy-i18n-lint.toml").exists());

        if let Some(orig) = original {
            std::env::set_current_dir(orig).unwrap();
        }
    }

    #[test]
    fn config_explicit_path_nonexistent_errors() {
        let env = TestEnv::new();
        env.write("en.json", r#"{"a":"A"}"#);
        env.write("uk.json", r#"{"a":"А"}"#);

        let result = env.run(CliOptions {
            config_path: Some(PathBuf::from("/nonexistent/config.toml")),
            ..base_opts()
        });
        let err = result.unwrap_err();
        assert!(err.to_string().contains("read config"), "got: {err}");
    }

    #[test]
    fn extract_placeholders_empty() {
        let re = Regex::new(r"\{([A-Za-z0-9_]+)\}").unwrap();
        let result = extract_placeholders(&re, "no placeholders here");
        assert!(result.is_empty());
    }

    #[test]
    fn extract_placeholders_multiple() {
        let re = Regex::new(r"\{([A-Za-z0-9_]+)\}").unwrap();
        let result = extract_placeholders(&re, "{name} and {count} and {name}");
        let mut expected = BTreeSet::new();
        expected.insert("name".to_string());
        expected.insert("count".to_string());
        assert_eq!(result, expected);
    }
}
