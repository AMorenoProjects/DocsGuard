//! Parser de código fuente usando tree-sitter.
//!
//! Soporta múltiples lenguajes (TypeScript, Rust) y provee utilidades
//! compartidas para la extracción de anotaciones `@docs`.

use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::core::types::CodeEntity;
use crate::parser::lang;

/// Lenguajes soportados por el code parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    TypeScript,
    Rust,
}

impl Language {
    /// Detecta el lenguaje a partir de la extensión del archivo.
    pub fn from_extension(path: &Path) -> Result<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("ts" | "tsx") => Ok(Language::TypeScript),
            Some("js" | "jsx") => Ok(Language::TypeScript), // tree-sitter-typescript parsea JS
            Some("rs") => Ok(Language::Rust),
            Some(ext) => bail!(
                "Extensión '.{}' no soportada.\n    -> Lenguajes soportados: TypeScript (.ts/.tsx), Rust (.rs)",
                ext
            ),
            None => bail!(
                "El archivo '{}' no tiene extensión.\n    -> No se puede determinar el lenguaje.",
                path.display()
            ),
        }
    }
}

/// Tamaño máximo de archivo para prevenir DoS (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// @docs: [parse-code-file]
/// Parsea un archivo de código auto-detectando el lenguaje por extensión.
pub fn parse_code_file(file_path: &Path) -> Result<Vec<CodeEntity>> {
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

    let language = Language::from_extension(file_path)?;

    match language {
        Language::TypeScript => lang::typescript::parse_typescript_source(&source, file_path),
        Language::Rust => lang::rust::parse_rust_source(&source, file_path),
    }
}

/// Busca la anotación `/// @docs: [id]` en los comentarios previos a un nodo.
///
/// `comment_kind` varía según el lenguaje:
/// - TypeScript: `"comment"`
/// - Rust: `"line_comment"`
pub fn find_docs_annotation(
    func_node: &tree_sitter::Node,
    source: &[u8],
    parent_node: &tree_sitter::Node,
    comment_kind: &str,
) -> Option<String> {
    let func_start = func_node.start_position().row;

    let mut cursor = parent_node.walk();
    let siblings: Vec<_> = parent_node.children(&mut cursor).collect();

    // Rastrear la fila del nodo anterior para medir gaps entre comentarios
    // consecutivos, no desde la función (que puede estar lejos si hay
    // múltiples líneas de doc-comments).
    let mut prev_row = func_start;

    for sibling in siblings.iter().rev() {
        let sibling_start_row = sibling.start_position().row;

        // Solo mirar nodos que comiencen antes de la función
        if sibling_start_row >= func_start {
            continue;
        }

        // Si hay más de una línea vacía entre este nodo y el anterior, dejar de buscar
        if prev_row.saturating_sub(sibling_start_row) > 2 {
            break;
        }

        prev_row = sibling_start_row;

        if sibling.kind() == comment_kind {
            if let Ok(text) = sibling.utf8_text(source) {
                if let Some(id) = extract_docs_id_from_comment(text) {
                    return Some(id);
                }
            }
        }

        // Si encontramos algo que no es un comentario, dejar de buscar
        if sibling.kind() != comment_kind {
            break;
        }
    }

    None
}

/// Extrae el ID de una anotación `/// @docs: [id]` o `// @docs: [id]`.
pub fn extract_docs_id_from_comment(comment: &str) -> Option<String> {
    let trimmed = comment.trim();
    let content = if let Some(rest) = trimmed.strip_prefix("///") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("//") {
        rest
    } else {
        return None;
    };

    let content = content.trim();

    if let Some(after_docs) = content.strip_prefix("@docs:") {
        let id_part = after_docs.trim();
        let id = if id_part.starts_with('[') && id_part.ends_with(']') {
            &id_part[1..id_part.len() - 1]
        } else {
            id_part
        };
        let id = id.trim();
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extract_docs_id_triple_slash_brackets() {
        assert_eq!(
            extract_docs_id_from_comment("/// @docs: [auth-login]"),
            Some("auth-login".into())
        );
    }

    #[test]
    fn extract_docs_id_double_slash_no_brackets() {
        assert_eq!(
            extract_docs_id_from_comment("// @docs: auth-login"),
            Some("auth-login".into())
        );
    }

    #[test]
    fn extract_docs_id_irrelevant_comment() {
        assert_eq!(
            extract_docs_id_from_comment("// esto es un comentario"),
            None
        );
    }

    #[test]
    fn language_detection_typescript() {
        assert_eq!(
            Language::from_extension(&PathBuf::from("foo.ts")).unwrap(),
            Language::TypeScript
        );
        assert_eq!(
            Language::from_extension(&PathBuf::from("bar.tsx")).unwrap(),
            Language::TypeScript
        );
    }

    #[test]
    fn language_detection_rust() {
        assert_eq!(
            Language::from_extension(&PathBuf::from("main.rs")).unwrap(),
            Language::Rust
        );
    }

    #[test]
    fn language_detection_unsupported() {
        assert!(Language::from_extension(&PathBuf::from("style.css")).is_err());
    }
}
