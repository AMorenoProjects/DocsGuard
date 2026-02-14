//! Parser de documentación Markdown usando pulldown-cmark.
//!
//! Extrae secciones marcadas con `<!-- @docs-id: xxx -->` y los argumentos
//! documentados dentro de cada sección. No usa regex para parsear estructura
//! Markdown (Blueprint §7: "No Regex Parser").

use anyhow::{Context, Result};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::path::Path;

use crate::core::types::{Arg, DocSection};

/// Tamaño máximo de archivo para prevenir DoS (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// @docs: [parse-markdown-file]
/// Parsea un archivo Markdown y extrae todas las secciones con anotación `@docs-id`.
pub fn parse_markdown_file(file_path: &Path) -> Result<Vec<DocSection>> {
    let metadata = std::fs::metadata(file_path)
        .with_context(|| format!("No se pudo leer metadata: {}", file_path.display()))?;
    if metadata.len() > MAX_FILE_SIZE {
        anyhow::bail!(
            "Archivo demasiado grande ({:.1} MB, máximo: {} MB): {}",
            metadata.len() as f64 / (1024.0 * 1024.0),
            MAX_FILE_SIZE / (1024 * 1024),
            file_path.display()
        );
    }
    let source = std::fs::read_to_string(file_path)
        .with_context(|| format!("No se pudo leer el archivo: {}", file_path.display()))?;

    parse_markdown_source(&source, file_path)
}

/// Parsea Markdown desde un string (útil para testing).
pub fn parse_markdown_source(source: &str, file_path: &Path) -> Result<Vec<DocSection>> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(source, options);

    let mut sections: Vec<DocSection> = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_title: Option<String> = None;
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut current_args: Vec<Arg> = Vec::new();
    let mut current_line: usize = 0;

    // Estado para parseo de listas (Strategy Pattern: ListStrategy)
    let mut in_list_item = false;
    let mut list_item_text = String::new();

    // Estado para parseo de definiciones (Strategy Pattern: DefinitionStrategy)
    let mut in_paragraph = false;
    let mut paragraph_text = String::new();

    // Estado para parseo de tablas (Strategy Pattern: TableStrategy)
    let mut table_row: Vec<String> = Vec::new();
    let mut table_headers: Vec<String> = Vec::new();
    let mut in_table_head = false;
    let mut in_table_cell = false;
    let mut cell_text = String::new();

    // Calcular mapeo de offset a línea
    let line_offsets = build_line_offsets(source);

    for (event, range) in parser.into_offset_iter() {
        let line = offset_to_line(&line_offsets, range.start);

        match event {
            Event::Html(html) => {
                let html_str = html.trim();
                if let Some(id) = extract_docs_id_from_html(html_str) {
                    // Si ya teníamos una sección abierta, cerrarla
                    if let Some(prev_id) = current_id.take() {
                        sections.push(DocSection {
                            id: prev_id,
                            title: current_title.take(),
                            args: std::mem::take(&mut current_args),
                            file_path: file_path.to_path_buf(),
                            line: current_line,
                        });
                    }
                    current_id = Some(id);
                    current_line = line;
                }
            }

            // --- Headings ---
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                if current_id.is_some() && current_title.is_none() {
                    current_title = Some(heading_text.trim().to_string());
                }
            }

            // --- Párrafos (DefinitionStrategy) ---
            Event::Start(Tag::Paragraph) => {
                in_paragraph = true;
                paragraph_text.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                in_paragraph = false;
                if current_id.is_some() && !in_list_item {
                    for line in paragraph_text.lines() {
                        if let Some(arg) = parse_definition_as_arg(line) {
                            current_args.push(arg);
                        }
                    }
                }
            }

            // --- Listas (ListStrategy) ---
            Event::Start(Tag::Item) => {
                in_list_item = true;
                list_item_text.clear();
            }
            Event::End(TagEnd::Item) => {
                in_list_item = false;
                if current_id.is_some() {
                    if let Some(arg) = parse_list_item_as_arg(&list_item_text) {
                        current_args.push(arg);
                    }
                }
            }

            // --- Tablas (TableStrategy) ---
            Event::Start(Tag::Table(_)) => {
                table_headers.clear();
            }
            Event::End(TagEnd::Table) => {}
            Event::Start(Tag::TableHead) => {
                in_table_head = true;
                table_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                in_table_head = false;
                table_headers = table_row.clone();
                table_row.clear();
            }
            Event::Start(Tag::TableRow) => {
                table_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                if !in_table_head && current_id.is_some() && !table_row.is_empty() {
                    if let Some(arg) = parse_table_row_as_arg(&table_headers, &table_row) {
                        current_args.push(arg);
                    }
                }
            }
            Event::Start(Tag::TableCell) => {
                in_table_cell = true;
                cell_text.clear();
            }
            Event::End(TagEnd::TableCell) => {
                in_table_cell = false;
                table_row.push(cell_text.trim().to_string());
            }

            // --- Texto ---
            Event::SoftBreak | Event::HardBreak => {
                if in_paragraph {
                    paragraph_text.push('\n');
                }
            }

            Event::Text(text) => {
                if in_heading {
                    heading_text.push_str(&text);
                } else if in_list_item {
                    list_item_text.push_str(&text);
                } else if in_table_cell {
                    cell_text.push_str(&text);
                } else if in_paragraph {
                    paragraph_text.push_str(&text);
                }
            }

            Event::Code(code) => {
                if in_heading {
                    heading_text.push_str(&code);
                } else if in_list_item {
                    list_item_text.push('`');
                    list_item_text.push_str(&code);
                    list_item_text.push('`');
                } else if in_table_cell {
                    cell_text.push_str(&code);
                } else if in_paragraph {
                    paragraph_text.push('`');
                    paragraph_text.push_str(&code);
                    paragraph_text.push('`');
                }
            }

            _ => {}
        }
    }

    // Cerrar última sección si existe
    if let Some(id) = current_id.take() {
        sections.push(DocSection {
            id,
            title: current_title.take(),
            args: std::mem::take(&mut current_args),
            file_path: file_path.to_path_buf(),
            line: current_line,
        });
    }

    Ok(sections)
}

/// Extrae el ID de un comentario HTML `<!-- @docs-id: xxx -->`.
fn extract_docs_id_from_html(html: &str) -> Option<String> {
    let content = html.strip_prefix("<!--")?.strip_suffix("-->")?;
    let content = content.trim();
    let after_prefix = content.strip_prefix("@docs-id:")?;
    let id = after_prefix.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Parsea un ítem de lista como argumento documentado.
/// Formatos soportados:
///   - `name: description`
///   - `name` (`type`): description
///   - `name` (type) — description
fn parse_list_item_as_arg(text: &str) -> Option<Arg> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Descartar ítems con formato rich (negrita, etc.) — no son argumentos
    if text.starts_with("**") || text.starts_with('*') && !text.starts_with("* ") {
        return None;
    }

    // Intentar formato: `name` (`type`): description
    // o: name (type): description
    // o simplemente: name: description
    let (name_part, rest) = if let Some(stripped) = text.strip_prefix('`') {
        // Buscar cierre de backtick
        let end = stripped.find('`')?;
        let name = &stripped[..end];
        (name, &stripped[end + 1..])
    } else if let Some(colon_pos) = text.find(':') {
        let name = text[..colon_pos].trim();
        // Verificar si hay tipo entre paréntesis en el nombre
        if let Some(paren_pos) = name.find('(') {
            (&text[..paren_pos], &text[paren_pos..])
        } else {
            (name, &text[colon_pos..])
        }
    } else {
        return None;
    };

    let name = name_part.trim().to_string();
    if name.is_empty() {
        return None;
    }

    // Extraer tipo si hay paréntesis
    let rest = rest.trim();
    let (type_name, description) = if rest.starts_with('(') {
        if let Some(close_paren) = rest.find(')') {
            let type_str = rest[1..close_paren].trim();
            // Limpiar backticks del tipo
            let type_str = type_str.trim_matches('`');
            let desc = rest[close_paren + 1..].trim();
            let desc = desc.strip_prefix(':').unwrap_or(desc);
            let desc = desc.strip_prefix('—').unwrap_or(desc);
            let desc = desc.strip_prefix('-').unwrap_or(desc);
            (Some(type_str.to_string()), Some(desc.trim().to_string()))
        } else {
            (None, Some(rest.to_string()))
        }
    } else if let Some(stripped) = rest.strip_prefix(':') {
        let desc = stripped.trim();
        (None, Some(desc.to_string()))
    } else {
        (
            None,
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            },
        )
    };

    Some(Arg {
        name,
        type_name,
        description: description.filter(|d| !d.is_empty()),
    })
}

/// Parsea una fila de tabla como argumento documentado.
/// Espera columnas con headers como: Name/Param, Type, Description.
fn parse_table_row_as_arg(headers: &[String], row: &[String]) -> Option<Arg> {
    if row.is_empty() {
        return None;
    }

    let find_col = |names: &[&str]| -> Option<usize> {
        headers.iter().position(|h| {
            let lower = h.to_lowercase();
            names.iter().any(|n| lower.contains(n))
        })
    };

    let name_col = find_col(&["name", "param", "arg", "nombre"]).unwrap_or(0);
    let type_col = find_col(&["type", "tipo"]);
    let desc_col = find_col(&["desc", "descripción", "description"]);

    let name = row.get(name_col)?.trim().trim_matches('`').to_string();
    if name.is_empty() {
        return None;
    }

    let type_name = type_col
        .and_then(|i| row.get(i))
        .map(|t| t.trim().trim_matches('`').to_string())
        .filter(|t| !t.is_empty());

    let description = desc_col
        .and_then(|i| row.get(i))
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    Some(Arg {
        name,
        type_name,
        description,
    })
}

/// Parsea una línea de párrafo como argumento en formato definición.
/// Formatos soportados (DefinitionStrategy, Blueprint §4.2):
///   - `name`: description
///   - `name` (`type`): description
///   - name (type): description
///
/// Solo matchea si la línea empieza con backtick-wrapped name o un identificador
/// seguido de `:` o `(type):`, para evitar falsos positivos con prosa normal.
fn parse_definition_as_arg(line: &str) -> Option<Arg> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Solo matchear si empieza con backtick (formato `name`: ...) para evitar
    // falsos positivos con prosa normal que contenga dos puntos.
    if let Some(stripped) = line.strip_prefix('`') {
        let end = stripped.find('`')?;
        let name = stripped[..end].trim();
        if name.is_empty() || name.contains(' ') {
            return None;
        }

        let rest = stripped[end + 1..].trim();

        // Extraer tipo si hay paréntesis
        let (type_name, description) = if rest.starts_with('(') {
            if let Some(close_paren) = rest.find(')') {
                let type_str = rest[1..close_paren].trim().trim_matches('`');
                let desc = rest[close_paren + 1..].trim();
                let desc = desc.strip_prefix(':').unwrap_or(desc);
                let desc = desc.strip_prefix('—').unwrap_or(desc);
                let desc = desc.strip_prefix('-').unwrap_or(desc);
                (Some(type_str.to_string()), Some(desc.trim().to_string()))
            } else {
                (None, Some(rest.to_string()))
            }
        } else if let Some(stripped) = rest.strip_prefix(':') {
            (None, Some(stripped.trim().to_string()))
        } else {
            return None; // No separator after name — not a definition
        };

        Some(Arg {
            name: name.to_string(),
            type_name,
            description: description.filter(|d| !d.is_empty()),
        })
    } else {
        None
    }
}

/// Construye un índice de offsets por línea para convertir byte offset → número de línea.
fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convierte un byte offset a número de línea (1-based).
fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line + 1,
        Err(line) => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extract_id_from_html_comment() {
        assert_eq!(
            extract_docs_id_from_html("<!-- @docs-id: auth-login -->"),
            Some("auth-login".into())
        );
    }

    #[test]
    fn extract_id_extra_whitespace() {
        assert_eq!(
            extract_docs_id_from_html("<!--  @docs-id:  user-create  -->"),
            Some("user-create".into())
        );
    }

    #[test]
    fn extract_id_not_docs() {
        assert_eq!(extract_docs_id_from_html("<!-- just a comment -->"), None);
    }

    #[test]
    fn parse_section_with_id_and_heading() {
        let source = r#"
# API Reference

<!-- @docs-id: auth-login -->
## Login

Authenticates a user.
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].id, "auth-login");
        assert_eq!(sections[0].title.as_deref(), Some("Login"));
    }

    #[test]
    fn parse_section_with_list_args() {
        let source = r#"
<!-- @docs-id: auth-login -->
## Login

Arguments:
- username: The user's login name
- password: The user's password
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].args.len(), 2);
        assert_eq!(sections[0].args[0].name, "username");
        assert_eq!(
            sections[0].args[0].description.as_deref(),
            Some("The user's login name")
        );
    }

    #[test]
    fn parse_section_with_table_args() {
        let source = r#"
<!-- @docs-id: user-create -->
## Create User

| Param | Type | Description |
|-------|------|-------------|
| name | string | The user's display name |
| email | string | The user's email address |
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].args.len(), 2);
        assert_eq!(sections[0].args[0].name, "name");
        assert_eq!(sections[0].args[0].type_name.as_deref(), Some("string"));
        assert_eq!(sections[0].args[1].name, "email");
    }

    #[test]
    fn parse_section_with_definition_args() {
        let source = r#"
<!-- @docs-id: user-update -->
## Update User

Parameters:

`name` (`String`): The user's display name
`email` (`String`): The user's email address
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].args.len(), 2);
        assert_eq!(sections[0].args[0].name, "name");
        assert_eq!(sections[0].args[0].type_name.as_deref(), Some("String"));
        assert_eq!(
            sections[0].args[0].description.as_deref(),
            Some("The user's display name")
        );
        assert_eq!(sections[0].args[1].name, "email");
    }

    #[test]
    fn parse_definition_without_type() {
        let source = r#"
<!-- @docs-id: config-set -->
## Set Config

`key`: The configuration key
`value`: The configuration value
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].args.len(), 2);
        assert_eq!(sections[0].args[0].name, "key");
        assert!(sections[0].args[0].type_name.is_none());
        assert_eq!(
            sections[0].args[0].description.as_deref(),
            Some("The configuration key")
        );
    }

    #[test]
    fn definition_ignores_plain_prose() {
        let source = r#"
<!-- @docs-id: test-fn -->
## Test

This function does something: it processes data.
Note: this is just a description paragraph.
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 1);
        // Prose with colons should NOT be parsed as arguments
        assert!(sections[0].args.is_empty());
    }

    #[test]
    fn parse_multiple_sections() {
        let source = r#"
<!-- @docs-id: auth-login -->
## Login

Login function.

<!-- @docs-id: auth-logout -->
## Logout

Logout function.
"#;
        let sections = parse_markdown_source(source, &PathBuf::from("docs/api.md")).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].id, "auth-login");
        assert_eq!(sections[1].id, "auth-logout");
    }
}
