<!-- File: AGENTS.md -->
# AI Agent Rules (bevy-i18n-lint)

## Goal
Keep the CLI predictable and CI-friendly.

## Non-negotiables
- Always run: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`
- Preserve stable CLI flags and output formats
- Any new output fields must be additive (no breaking changes in JSON)

## Repo conventions
- No breaking changes without bumping major version
- Keep output deterministic (sorted keys, stable ordering)
- Keep dependencies minimal

## Tasks agent can do safely
- Add a new check behind a flag
- Improve JSON output schema (add new fields only)
- Add more fixtures and tests
- Improve error messages

## Do-not-do
- Do not change default exit code rules without a migration note in README
- Do not change default directory/base assumptions

## How to implement changes
1) Update code
2) Add tests for the new behavior
3) Update README usage examples if new flags are added

## Quick commands
- Test: `cargo test`
- Lint: `cargo clippy -- -D warnings`
- Format: `cargo fmt`
