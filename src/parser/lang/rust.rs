//! Parser de Rust usando tree-sitter.
//!
//! Extrae funciones de archivos Rust y busca anotaciones `/// @docs: [id]`
//! en los doc-comments inmediatamente anteriores a la declaración.

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

use crate::core::types::{Arg, CodeEntity};
use crate::parser::code_parser::find_docs_annotation;

/// Parsea código Rust desde un string.
pub fn parse_rust_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&language.into())
        .context("Error al configurar tree-sitter con Rust")?;

    let tree = parser
        .parse(source, None)
        .context("Error al parsear el archivo Rust")?;

    let root_node = tree.root_node();
    let source_bytes = source.as_bytes();
    let mut entities = Vec::new();

    collect_functions(&root_node, source_bytes, file_path, &mut entities)?;

    Ok(entities)
}

/// Recorre el AST recursivamente buscando `function_item` nodes.
fn collect_functions(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    entities: &mut Vec<CodeEntity>,
) -> Result<()> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_item" => {
                if let Some(entity) = extract_function(&child, source, file_path, node)? {
                    entities.push(entity);
                }
            }
            // Recurrir en módulos, impl blocks, etc.
            "mod_item" | "impl_item" | "trait_item" => {
                if let Some(body) = child.child_by_field_name("body") {
                    collect_functions(&body, source, file_path, entities)?;
                }
            }
            _ => {
                collect_functions(&child, source, file_path, entities)?;
            }
        }
    }

    Ok(())
}

/// Extrae una CodeEntity de un nodo `function_item` de Rust.
fn extract_function(
    func_node: &tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    parent_node: &tree_sitter::Node,
) -> Result<Option<CodeEntity>> {
    let name = func_node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(String::from);

    let name = match name {
        Some(n) => n,
        None => return Ok(None),
    };

    let args = extract_parameters(func_node, source)?;
    let return_type = extract_return_type(func_node, source);

    // En Rust, los doc comments `///` son nodos `line_comment` en tree-sitter
    let doc_id = find_docs_annotation(func_node, source, parent_node, "line_comment");

    let line = func_node.start_position().row + 1;

    Ok(Some(CodeEntity {
        name,
        args,
        return_type,
        doc_id,
        file_path: file_path.to_path_buf(),
        line,
    }))
}

/// Extrae los parámetros de una función Rust.
/// En tree-sitter-rust, los parámetros están en el nodo `parameters`.
/// Cada parámetro es un `parameter` con campos `pattern` y `type`.
fn extract_parameters(func_node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Arg>> {
    let mut args = Vec::new();

    let params_node = match func_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return Ok(args),
    };

    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if child.kind() == "parameter" {
            let param_name = child
                .child_by_field_name("pattern")
                .and_then(|n| n.utf8_text(source).ok())
                .map(String::from)
                .unwrap_or_default();

            let type_name = child
                .child_by_field_name("type")
                .and_then(|n| n.utf8_text(source).ok())
                .map(String::from);

            if !param_name.is_empty() {
                args.push(Arg {
                    name: param_name,
                    type_name,
                    description: None,
                });
            }
        } else if child.kind() == "self_parameter" {
            // Ignorar `self`, `&self`, `&mut self` — no es un argumento documentable
        }
    }

    Ok(args)
}

/// Extrae el tipo de retorno de una función Rust.
/// En tree-sitter-rust, el campo es `return_type` y contiene `-> Type`.
fn extract_return_type(func_node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    func_node
        .child_by_field_name("return_type")
        .and_then(|n| n.utf8_text(source).ok())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_simple_rust_function() {
        let source = r#"
/// @docs: [validate-links]
pub fn validate_links(entities: &[CodeEntity], sections: &[DocSection]) -> Vec<ValidationResult> {
    vec![]
}
"#;
        let entities = parse_rust_source(source, &PathBuf::from("test.rs")).unwrap();
        assert_eq!(entities.len(), 1);

        let entity = &entities[0];
        assert_eq!(entity.name, "validate_links");
        assert_eq!(entity.doc_id, Some("validate-links".into()));
        assert_eq!(entity.args.len(), 2);
        assert_eq!(entity.args[0].name, "entities");
        assert_eq!(entity.args[0].type_name.as_deref(), Some("&[CodeEntity]"));
        assert_eq!(entity.args[1].name, "sections");
    }

    #[test]
    fn parse_rust_function_without_annotation() {
        let source = r#"
fn helper() {
}
"#;
        let entities = parse_rust_source(source, &PathBuf::from("test.rs")).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "helper");
        assert_eq!(entities[0].doc_id, None);
    }

    #[test]
    fn parse_rust_function_with_return_type() {
        let source = r#"
/// @docs: [parse-file]
pub fn parse_file(path: &Path) -> Result<Vec<CodeEntity>> {
    Ok(vec![])
}
"#;
        let entities = parse_rust_source(source, &PathBuf::from("test.rs")).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(
            entity_return_type(&entities[0]),
            Some("Result<Vec<CodeEntity>>")
        );
    }

    #[test]
    fn parse_rust_method_with_self() {
        let source = r#"
impl Validator {
    /// @docs: [validator-run]
    pub fn run(&self, input: &str) -> bool {
        true
    }
}
"#;
        let entities = parse_rust_source(source, &PathBuf::from("test.rs")).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "run");
        assert_eq!(entities[0].doc_id, Some("validator-run".into()));
        // &self no debe aparecer como argumento
        assert_eq!(entities[0].args.len(), 1);
        assert_eq!(entities[0].args[0].name, "input");
    }

    #[test]
    fn parse_multiple_rust_functions() {
        let source = r#"
/// @docs: [fn-a]
pub fn alpha(x: i32) -> i32 { x }

fn private_helper() {}

/// @docs: [fn-b]
pub fn beta(name: String) -> String { name }
"#;
        let entities = parse_rust_source(source, &PathBuf::from("test.rs")).unwrap();
        assert_eq!(entities.len(), 3);
        assert_eq!(entities[0].doc_id, Some("fn-a".into()));
        assert_eq!(entities[1].doc_id, None);
        assert_eq!(entities[2].doc_id, Some("fn-b".into()));
    }

    fn entity_return_type(entity: &CodeEntity) -> Option<&str> {
        entity.return_type.as_deref()
    }
}
