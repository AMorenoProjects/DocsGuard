//! Parser de código fuente usando tree-sitter.
//!
//! Soporta múltiples lenguajes (TypeScript, Rust, Python, Go, Java, C#) y provee utilidades
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
    Python,
    Go,
    Java,
    CSharp,
}

impl Language {
    /// Detecta el lenguaje a partir de la extensión del archivo.
    pub fn from_extension(path: &Path) -> Result<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("ts" | "tsx") => Ok(Language::TypeScript),
            Some("js" | "jsx") => Ok(Language::TypeScript), // tree-sitter-typescript parsea JS
            Some("rs") => Ok(Language::Rust),
            Some("py") => Ok(Language::Python),
            Some("go") => Ok(Language::Go),
            Some("java") => Ok(Language::Java),
            Some("cs") => Ok(Language::CSharp),
            Some(ext) => bail!(
                "Extensión '.{}' no soportada.\n    -> Lenguajes soportados: TypeScript (.ts/.tsx), Rust (.rs), Python (.py), Go (.go), Java (.java), C# (.cs)",
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

/// Inicializa un Parser de tree-sitter y parsea el source en un solo paso.
///
/// Refactorizado: función DRY compartida por todos los parsers de lenguaje —
/// elimina ~10 líneas de boilerplate idéntico en cada módulo.
pub fn create_tree(
    source: &str,
    language: tree_sitter::Language,
    lang_name: &str,
) -> Result<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .with_context(|| format!("Error al configurar tree-sitter con {}", lang_name))?;
    parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Error al parsear el archivo {}", lang_name))
}

/// Verifica que un archivo existe y retorna un error educativo si no.
///
/// Refactorizado: función DRY compartida por todos los comandos CLI —
/// elimina el patrón repetido `if !path.exists() { bail!(...) }`.
pub fn require_file_exists(path: &Path, kind: &str) -> Result<()> {
    if !path.exists() {
        anyhow::bail!(
            "Archivo de {} no encontrado: {}\n    -> Verifica que la ruta sea correcta.",
            kind,
            path.display()
        );
    }
    Ok(())
}

/// @docs: [parse-code-file]
/// Parsea un archivo de código auto-detectando el lenguaje por extensión.
pub fn parse_code_file(file_path: &Path) -> Result<Vec<CodeEntity>> {
    use std::io::Read;
    // VUL-03: abrir una sola vez — el check de tamaño y la lectura comparten el mismo fd,
    // eliminando la ventana TOCTOU entre metadata() y read_to_string().
    let mut file = std::fs::File::open(file_path)
        .with_context(|| format!("No se pudo abrir: {}", file_path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("No se pudo leer metadata: {}", file_path.display()))?;
    if metadata.len() > MAX_FILE_SIZE {
        anyhow::bail!(
            "Archivo demasiado grande ({:.1} MB, máximo: {} MB): {}",
            metadata.len() as f64 / (1024.0 * 1024.0),
            MAX_FILE_SIZE / (1024 * 1024),
            file_path.display()
        );
    }
    let mut source = String::with_capacity(metadata.len() as usize);
    file.read_to_string(&mut source)
        .with_context(|| format!("No se pudo leer el archivo: {}", file_path.display()))?;

    let language = Language::from_extension(file_path)?;

    match language {
        Language::TypeScript => lang::typescript::parse_typescript_source(&source, file_path),
        Language::Rust => lang::rust::parse_rust_source(&source, file_path),
        Language::Python => lang::python::parse_python_source(&source, file_path),
        Language::Go => lang::go::parse_go_source(&source, file_path),
        Language::Java => lang::java::parse_java_source(&source, file_path),
        Language::CSharp => lang::c_sharp::parse_c_sharp_source(&source, file_path),
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

/// Valida que un ID de sección solo contiene caracteres seguros.
///
/// Solo se permiten: `[a-zA-Z0-9_-]`. Previene inyección de código (VUL-01):
/// un ID con `\n` en el interior podría inyectar líneas arbitrarias al escribir
/// anotaciones `@docs` en archivos de código fuente.
pub(crate) fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Escribe `content` en `path` de forma atómica usando temp file + POSIX rename.
///
/// Función compartida usada por `apply_changes` (interactive) y `Baseline::save`.
/// Previene TOCTOU, corrupción parcial y symlink attacks (VUL-02).
pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp_path = path.with_extension("tmp.docsguardwrite");
    std::fs::write(&tmp_path, content)
        .with_context(|| format!("No se pudo escribir temporal: {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path)
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp_path);
        })
        .with_context(|| format!("No se pudo renombrar a: {}", path.display()))
}

/// Devuelve el path como string con secuencias ANSI escapadas.
///
/// Previene terminal injection (VUL-05): paths con `\x1b[...` podrían manipular
/// la apariencia del terminal y engañar al usuario.
pub(crate) fn safe_display(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\x1b', "\\x1b")
        .replace('\r', "\\r")
}

/// Extrae el ID de una anotación `/// @docs: [id]`, `// @docs: [id]`, o `# @docs: [id]`.
pub fn extract_docs_id_from_comment(comment: &str) -> Option<String> {
    let trimmed = comment.trim();
    let content = if let Some(rest) = trimmed.strip_prefix("///") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("//") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("#") {
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
        // VUL-01: rechazar IDs con caracteres fuera de [a-zA-Z0-9_-]
        if is_valid_id(id) {
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

    // VUL-01: inyección de código via section_id con newlines
    #[test]
    fn extract_docs_id_rejects_newline_injection() {
        // Un doc malicioso podría intentar: @docs: [legit-id\nfn evil() {}]
        assert_eq!(
            extract_docs_id_from_comment("/// @docs: [legit-id\nfn evil() {}]"),
            None
        );
    }

    #[test]
    fn extract_docs_id_rejects_spaces() {
        assert_eq!(
            extract_docs_id_from_comment("/// @docs: [id with spaces]"),
            None
        );
    }

    #[test]
    fn is_valid_id_allows_alphanumeric_and_separators() {
        assert!(is_valid_id("auth-login"));
        assert!(is_valid_id("user_create"));
        assert!(is_valid_id("parseCodeFile123"));
    }

    #[test]
    fn is_valid_id_rejects_injection_chars() {
        assert!(!is_valid_id("id\ninjected"));
        assert!(!is_valid_id("id with space"));
        assert!(!is_valid_id("id;evil()"));
        assert!(!is_valid_id(""));
    }
}
