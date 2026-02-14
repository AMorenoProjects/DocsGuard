# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-14

### Added
- `check` command — validates links between code and documentation
- `scaffold` command — interactive TUI for linking functions to doc sections
- `watch` command — real-time file watching with automatic re-validation
- `baseline` command — dump existing errors for "Green Build Day 1"
- TypeScript/JavaScript parser (tree-sitter)
- Rust parser (tree-sitter)
- Markdown parser with three documentation formats:
  - Table strategy (`| Param | Type | Description |`)
  - List strategy (`- param: description`)
  - Definition strategy (`` `param` (`type`): description ``)
- Ghost argument detection (documented but not in code)
- Missing argument detection (in code but not documented)
- Type mismatch detection with normalization (String/&str, i32/number, etc.)
- Levenshtein-based heuristic matching for scaffold suggestions
- Baseline system (`.docsguard/baseline.yaml`) for gradual adoption
- GitHub Actions CI workflow
- Docker support with multi-stage build
