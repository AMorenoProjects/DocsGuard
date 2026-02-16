<div align="center">

**Read this in other languages: [English](README.md)**

</div>

# DocsGuard

**Motor de Integridad de Documentación** — Elimina la deriva código-documentación mediante validación heurística, parsing multiformato y corrección interactiva.

[![CI](https://github.com/jandro/docsguard/actions/workflows/ci.yml/badge.svg)](https://github.com/jandro/docsguard/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## El Problema

La documentación se desincroniza del código en silencio. Los argumentos se renombran, los parámetros se añaden, los tipos cambian — y la documentación queda congelada en el tiempo. Para cuando alguien se da cuenta, el daño ya está hecho.

## La Solución

DocsGuard vincula funciones directamente con sus secciones de documentación y las valida automáticamente:

- **Validación de enlaces estáticos** — ¿Existe realmente la sección documentada?
- **Detección de argumentos fantasma** — ¿Hay algo documentado que no existe en el código?
- **Detección de argumentos faltantes** — ¿Hay parámetros en el código que no están en la documentación?
- **Detección de type mismatch** — ¿Coincide `string` en docs con `i32` en código?
- **Scaffold interactivo** — Nunca modifica código sin permiso explícito
- **Sistema de baseline** — Adopta en proyectos legacy con cero roturas de CI desde el Día 1

## Inicio Rápido

### Instalación

**Vía npm (Recomendado):**
```bash
npm install -g docsguard
```

**Desde el código fuente (Rust):**
```bash
cargo install --path .
```

### 1. Anota tu código

Añade anotaciones `@docs` encima de las funciones para vincularlas a la documentación:

```rust
/// @docs: [auth-login]
fn login(username: &str, password: &str) -> Result<Token> {
    // ...
}
```

### 2. Marca tu documentación

Añade marcadores invisibles en tu documentación Markdown:

```markdown
<!-- @docs-id: auth-login -->
## Login

Autentica un usuario con credenciales.

| Param    | Type   | Description               |
|----------|--------|---------------------------|
| username | string | Nombre de usuario         |
| password | string | Contraseña del usuario    |
```

### 3. Valida

```bash
docsguard check src/auth.rs docs/api.md
```

```
[i] Info en fn login (src/auth.rs:3)
    -> Enlace verificado: fn login <-> sección 'Login'

---
Resumen: 0 errores, 0 advertencias, 1 total
```

## Comandos

### `docsguard check <code_file> <doc_file>`

Valida los enlaces entre código y documentación. Sale con código 1 si hay errores (compatible con CI).

```bash
docsguard check src/main.rs docs/api.md
docsguard check src/main.rs docs/api.md --project-root .  # usar baseline
```

### `docsguard scaffold <code_file> <doc_file>`

TUI interactivo que sugiere enlaces entre funciones no vinculadas y secciones de docs usando heurística de Levenshtein (>80% de confianza).

```bash
docsguard scaffold src/main.rs docs/api.md              # interactivo
docsguard scaffold src/main.rs docs/api.md --dry-run     # solo previsualizar
docsguard scaffold src/main.rs docs/api.md --force        # aceptar todo
```

### `docsguard watch <code_file> <doc_file>`

Observa archivos en busca de cambios y re-valida automáticamente (<200ms de respuesta).

```bash
docsguard watch src/main.rs docs/api.md
```

### `docsguard baseline <code_file> <doc_file>`

Vuelca los errores actuales a `.docsguard/baseline.yaml` para que el CI pase inmediatamente. Solo se bloquearán regresiones *nuevas*.

```bash
docsguard baseline src/main.rs docs/api.md --project-root .
```

## Lenguajes Soportados

| Lenguaje   | Extensiones      | Parser      |
|------------|------------------|-------------|
| TypeScript | `.ts`, `.tsx`    | tree-sitter |
| JavaScript | `.js`, `.jsx`    | tree-sitter |
| Rust       | `.rs`            | tree-sitter |

## Formatos de Documentación

DocsGuard parsea tres formatos de documentación de argumentos automáticamente:

**Tablas:**
```markdown
| Param | Type | Description |
|-------|------|-------------|
| name  | string | El nombre del usuario |
```

**Listas:**
```markdown
- name: El nombre del usuario
- `email` (`string`): El email del usuario
```

**Definiciones:**
```markdown
`name` (`string`): El nombre del usuario
`email` (`string`): El email del usuario
```

## Docker

```bash
docker build -t docsguard .
docker run --rm -v $(pwd):/workspace docsguard check src/main.rs docs/api.md
```

## Integración CI

Añade a tu workflow de GitHub Actions:

```yaml
- name: DocsGuard Check
  run: |
    cargo install --path .
    docsguard check src/main.rs docs/api.md
```

O usa el baseline para adopción gradual:

```yaml
- name: DocsGuard Check (con baseline)
  run: |
    cargo install --path .
    docsguard check src/main.rs docs/api.md --project-root .
```

## Normalización de Tipos

DocsGuard normaliza los tipos antes de compararlos, por lo que estos se consideran equivalentes:

| Tipo en Código | Tipo en Docs | Normalizado |
|----------------|--------------|-------------|
| `&str`         | `String`     | string      |
| `i32`          | `number`     | number      |
| `bool`         | `Boolean`    | boolean     |
| `UUID`         | `String`     | string      |

## Arquitectura

```
src/
  main.rs                Punto de entrada CLI (clap)
  core/
    types.rs             Tipos de dominio: CodeEntity, DocSection, Arg, ValidationResult
    validator.rs         Validación de enlaces + chequeo de argumentos + type mismatch
    heuristic.rs         Matching basado en Levenshtein (strsim)
  parser/
    code_parser.rs       Detección de lenguaje + extracción de anotaciones @docs
    doc_parser.rs        pulldown-cmark: estrategias Tabla, Lista, Definición
    lang/
      typescript.rs      Parser tree-sitter TypeScript/JavaScript
      rust.rs            Parser tree-sitter Rust
  interactive/mod.rs     Scaffold TUI (dialoguer)
  watch/mod.rs           Modo watch de archivos (notify)
  baseline/mod.rs        Sistema de baseline (serde_yaml)
```

## Contribuir

Las contribuciones son bienvenidas. Consulta [CONTRIBUTING.md](CONTRIBUTING.md) para las directrices.

## Licencia

Licencia MIT. Ver [LICENSE](LICENSE) para más detalles.
