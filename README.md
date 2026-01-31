# bevy-i18n-lint

Lint Bevy localization files (`.json` / `.ron`) for:
- missing keys (compared to base language)
- extra keys
- placeholder mismatches like `{name}`

## Install

```bash
cargo install bevy-i18n-lint
```

Or install from source:

```bash
cargo install --path .
```

## Usage

Basic usage:

```bash
bevy-i18n-lint
```

This will:
- Look for locale files in `assets/i18n/` (default)
- Use `en` as the base language (default)
- Report missing keys in other languages
- Report extra keys in other languages
- Report placeholder mismatches

### Options

```bash
bevy-i18n-lint --help
```

Available flags:
- `--dir <PATH>`: Directory containing locale files (default: `assets/i18n`)
- `--base <LANG>`: Base language code (default: `en`)
- `--strict`: Exit with error on any issues (missing, extra, or placeholder mismatches)
- `--fail-on-extra`: Exit with error on extra keys
- `--fail-on-placeholder`: Exit with error on placeholder mismatches
- `--format <FORMAT>`: Output format (default: `text`, options: `json`, `github`)

### Examples

Check a custom directory:

```bash
bevy-i18n-lint --dir ./locales
```

Use a different base language:

```bash
bevy-i18n-lint --base uk
```

Enable strict mode (fail on any issue):

```bash
bevy-i18n-lint --strict
```

Get JSON output for CI/CD:

```bash
bevy-i18n-lint --format json
```

Use GitHub Actions annotations:

```bash
bevy-i18n-lint --format github
```

Fail only on missing keys:

```bash
bevy-i18n-lint --fail-on-placeholder=false --fail-on-extra=false
```

## Output Formats

### Text (default)

Human-readable text output showing all issues found.

```bash
bevy-i18n-lint --format text
```

Example:
```
bevy-i18n-lint: base=en, langs=en, uk, fr

missing keys: 2
  [uk] welcome_message -> assets/i18n/uk.ron
  [fr] goodbye_message -> assets/i18n/fr.json

summary: missing=2, extra=0, placeholder_mismatch=0
```

### JSON

Machine-readable JSON output for CI/CD pipelines.

```bash
bevy-i18n-lint --format json
```

Example:
```json
{
  "base": "en",
  "langs": ["en", "uk", "fr"],
  "missing": [
    {
      "kind": "missing_key",
      "lang": "uk",
      "key": "welcome_message",
      "file": "assets/i18n/uk.ron",
      "message": "key 'welcome_message' is missing (base: en)"
    }
  ],
  "extra": [],
  "placeholder_mismatch": [],
  "totals": {
    "missing": 1,
    "extra": 0,
    "placeholder_mismatch": 0
  }
}
```

### GitHub

GitHub Actions annotations format for direct integration with CI.

```bash
bevy-i18n-lint --format github
```

Example:
```
::error file=assets/i18n/uk.ron,line=1,col=1::key 'welcome_message' is missing (base: en)
bevy-i18n-lint: missing=1, extra=0, placeholder_mismatch=0
```

## Exit Codes

- `0`: No issues found
- `1`: Issues found (depending on flags)

The exit code behavior:
- Default: Fails on missing keys only
- `--strict`: Fails on any issues (missing, extra, or placeholder mismatches)
- `--fail-on-extra`: Also fails on extra keys
- `--fail-on-placeholder`: Also fails on placeholder mismatches

## GitHub Actions

Example workflow:

```yaml
name: Lint i18n files

on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install --path .
      - run: bevy-i18n-lint --format github
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## License

Dual licensed under MIT and Apache-2.0. See [LICENSE](LICENSE) for details.

## Support

See [SUPPORT.md](SUPPORT.md) for support options.
