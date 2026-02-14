<div align="center">

**Leer en otros idiomas: [Español](README.es.md)**

</div>

# DocsGuard

**Documentation Integrity Engine** — Eliminates code-documentation drift through heuristic validation, multi-format parsing, and interactive correction.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## The Problem

Documentation drifts from code silently. Arguments get renamed, parameters get added, types change — and the docs stay frozen in time. By the time someone notices, the damage is done.

## The Solution

DocsGuard links functions directly to their documentation sections and validates them automatically:

- **Static link validation** — Does the documented section actually exist?
- **Ghost argument detection** — Is something documented that doesn't exist in code?
- **Missing argument detection** — Is there a code parameter missing from docs?
- **Type mismatch detection** — Does `string` in docs match `i32` in code?
- **Interactive scaffold** — Never modifies code without explicit permission
- **Baseline system** — Adopt on legacy projects with zero CI breakage on Day 1

## Quick Start

### Install from source

```bash
cargo install --path .
```

### 1. Annotate your code

Add `@docs` annotations above functions to link them to documentation:

```rust
/// @docs: [auth-login]
fn login(username: &str, password: &str) -> Result<Token> {
    // ...
}
```

### 2. Mark your docs

Add invisible markers in your Markdown documentation:

```markdown
<!-- @docs-id: auth-login -->
## Login

Authenticates a user with credentials.

| Param    | Type   | Description          |
|----------|--------|----------------------|
| username | string | The user's login name |
| password | string | The user's password   |
```

### 3. Validate

```bash
docsguard check src/auth.rs docs/api.md
```

```
[i] Info en fn login (src/auth.rs:3)
    -> Enlace verificado: fn login <-> sección 'Login'

---
Resumen: 0 errores, 0 advertencias, 1 total
```

## Commands

### `docsguard check <doc_file> <code_files>...`

Validates links between code and documentation. Exits with code 1 if errors are found (CI-friendly).

```bash
docsguard check docs/api.md src/main.rs
docsguard check docs/api.md src/core/validator.rs src/parser/*.rs
docsguard check docs/api.md src/main.rs --project-root .  # use baseline
```

### `docsguard scaffold <code_file> <doc_file>`

Interactive TUI that suggests links between unlinked functions and doc sections using Levenshtein heuristics (>80% confidence).

```bash
docsguard scaffold src/main.rs docs/api.md              # interactive
docsguard scaffold src/main.rs docs/api.md --dry-run     # preview only
docsguard scaffold src/main.rs docs/api.md --force        # accept all
```

### `docsguard watch <code_file> <doc_file>`

Watches files for changes and re-validates automatically (<200ms response).

```bash
docsguard watch src/main.rs docs/api.md
```

### `docsguard baseline <code_file> <doc_file>`

Dumps current errors to `.docsguard/baseline.yaml` so CI passes immediately. Only *new* regressions will be blocked.

```bash
docsguard baseline src/main.rs docs/api.md --project-root .
```

## Supported Languages

| Language   | Extensions       | Parser      |
|------------|------------------|-------------|
| TypeScript | `.ts`, `.tsx`    | tree-sitter |
| JavaScript | `.js`, `.jsx`    | tree-sitter |
| Rust       | `.rs`            | tree-sitter |

## Documentation Formats

DocsGuard parses three argument documentation formats automatically:

**Tables:**
```markdown
| Param | Type | Description |
|-------|------|-------------|
| name  | string | The user's name |
```

**Lists:**
```markdown
- name: The user's name
- `email` (`string`): The user's email
```

**Definitions:**
```markdown
`name` (`string`): The user's name
`email` (`string`): The user's email
```

## Docker

```bash
docker build -t docsguard .
docker run --rm -v $(pwd):/workspace docsguard check src/main.rs docs/api.md
```

## CI Integration

Add to your GitHub Actions workflow:

```yaml
- name: DocsGuard Check
  run: |
    cargo install --path .
    docsguard check src/main.rs docs/api.md
```

Or use the baseline for gradual adoption:

```yaml
- name: DocsGuard Check (with baseline)
  run: |
    cargo install --path .
    docsguard check src/main.rs docs/api.md --project-root .
```

## Type Normalization

DocsGuard normalizes types before comparison, so these are considered equivalent:

| Code Type | Doc Type | Normalized |
|-----------|----------|------------|
| `&str`    | `String` | string     |
| `i32`     | `number` | number     |
| `bool`    | `Boolean`| boolean    |
| `UUID`    | `String` | string     |

## Architecture

```
src/
  main.rs                CLI entry point (clap)
  core/
    types.rs             Domain types: CodeEntity, DocSection, Arg, ValidationResult
    validator.rs         Link validation + argument checking + type mismatch
    heuristic.rs         Levenshtein-based matching (strsim)
  parser/
    code_parser.rs       Language detection + @docs annotation extraction
    doc_parser.rs        pulldown-cmark: Table, List, Definition strategies
    lang/
      typescript.rs      tree-sitter TypeScript/JavaScript parser
      rust.rs            tree-sitter Rust parser
  interactive/mod.rs     Scaffold TUI (dialoguer)
  watch/mod.rs           File watch mode (notify)
  baseline/mod.rs        Baseline system (serde_yaml)
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License. See [LICENSE](LICENSE) for details.
