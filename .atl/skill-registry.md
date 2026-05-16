# Skill Registry

This registry tracks the specialized skills and standards for the `escpos-viewer-pro` project.

## Compact Rules

### Rust Engineering Standards
- Use `cargo fmt` for formatting.
- Run `cargo clippy` for linting.
- Follow standard Rust naming conventions (snake_case for functions/variables, PascalCase for types).
- Use `eframe` for GUI components, following the central `App` pattern in `src/app.rs`.

### Testing Standards
- All new logic in `src/escpos.rs` must have accompanying unit tests in the `tests` module.
- Use `cargo test` to verify changes.
- Preference for regression tests for any bug fixed in parsing logic.

## User Skills

| Trigger | Skill | Description |
|---|---|---|
| `cargo test` | `test-runner` | Primary test execution tool. |
| `cargo clippy` | `linter` | Static analysis tool for Rust. |
| `cargo fmt` | `formatter` | Code formatting tool. |
