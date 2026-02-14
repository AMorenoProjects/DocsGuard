# DocsGuard — API Interna

Documentación de las funciones públicas principales del motor de validación.

---

<!-- @docs-id: validate-links -->
## validate_links

Valida la integridad de los enlaces entre entidades de código y secciones de documentación.

Realiza tres tipos de verificación:
1. **Enlace estático** — ¿El `doc_id` anotado en el código tiene una sección correspondiente en el markdown?
2. **Argumentos fantasma** — ¿Hay argumentos documentados que no existen en la firma de la función?
3. **Type mismatch** — ¿El tipo documentado coincide con el tipo en el código fuente?

**Argumentos:**

| Param | Type | Description |
|-------|------|-------------|
| code_entities | &[CodeEntity] | Lista de funciones extraídas del código fuente |
| doc_sections | &[DocSection] | Lista de secciones extraídas del markdown |

**Retorna:** `Vec<ValidationResult>` — Lista de hallazgos con severidad, mensaje y contexto accionable.

---

<!-- @docs-id: parse-code-file -->
## parse_code_file

Parsea un archivo de código fuente, auto-detectando el lenguaje por su extensión.

Soporta:
- TypeScript (`.ts`, `.tsx`)
- JavaScript (`.js`, `.jsx`)
- Rust (`.rs`)

**Argumentos:**

| Param | Type | Description |
|-------|------|-------------|
| file_path | &Path | Ruta al archivo de código fuente |

**Retorna:** `Result<Vec<CodeEntity>>` — Las funciones encontradas con sus anotaciones `@docs`.

---

<!-- @docs-id: parse-markdown-file -->
## parse_markdown_file

Parsea un archivo Markdown y extrae las secciones marcadas con `<!-- @docs-id: xxx -->`.

Soporta múltiples formatos de documentación de argumentos (Strategy Pattern):
Tablas (`| Param | Type | Description |`) y Listas (`- param: description`).

**Argumentos:**

| Param | Type | Description |
|-------|------|-------------|
| file_path | &Path | Ruta al archivo Markdown |

**Retorna:** `Result<Vec<DocSection>>` — Las secciones encontradas con sus argumentos normalizados.
