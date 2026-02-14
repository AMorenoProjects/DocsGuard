# Contributing to DocsGuard

Thank you for your interest in contributing!

## Development Setup

```bash
# Clone the repository
git clone https://github.com/jandro/docsguard.git
cd docsguard

# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy

# Format
cargo fmt
```

## Project Structure

See the [Architecture section](README.md#architecture) in the README for an overview of the codebase.

The full technical specification is in `docs/blueprint.md`.

## Guidelines

- **Tests required** for new features and bug fixes
- **`cargo clippy`** must pass with zero warnings
- **`cargo fmt`** must be applied before committing
- **Educational error messages** — errors should include file location, context, and actionable suggestions
- **No regex for Markdown parsing** — use `pulldown-cmark` only
- **Trust-first** — never modify user files without explicit permission

## Adding Language Support

To add a new language parser:

1. Create `src/parser/lang/<language>.rs`
2. Implement a `parse_<language>_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>>` function
3. Add the language to the `Language` enum in `src/parser/code_parser.rs`
4. Add extension detection in `Language::from_extension()`
5. Add the dispatch in `parse_code_file()`
6. Add tests

## Commit Messages

Use conventional commits when possible:

- `feat: add Python language support`
- `fix: correct line number off-by-one in Rust parser`
- `docs: update README with new command`
- `test: add edge case tests for doc parser`

## Reporting Issues

When reporting bugs, please include:

- DocsGuard version (`docsguard --version`)
- OS and Rust version
- Minimal reproduction case (code file + doc file)
- Expected vs actual output
