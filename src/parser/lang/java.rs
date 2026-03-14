//! Parser para Java usando tree-sitter.
//!
//! Extrae funciones de archivos Java y busca anotaciones `// @docs: [id]`
//! en los comentarios inmediatamente anteriores a la declaración.

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

use crate::core::types::{Arg, CodeEntity};
use crate::parser::code_parser::find_docs_annotation;

/// Parsea código Java desde un string.
pub fn parse_java_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>> {
    let mut parser = Parser::new();
    let language = tree_sitter_java::LANGUAGE;
    parser
        .set_language(&language.into())
        .context("Error al configurar tree-sitter con Java")?;

    let tree = parser
        .parse(source, None)
        .context("Error al parsear el archivo Java")?;

    let root_node = tree.root_node();
    let source_bytes = source.as_bytes();
    let mut entities = Vec::new();

    collect_functions(&root_node, source_bytes, file_path, &mut entities)?;

    Ok(entities)
}

/// Recorre el AST recursivamente buscando `method_declaration` y `constructor_declaration`.
fn collect_functions(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    entities: &mut Vec<CodeEntity>,
) -> Result<()> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "method_declaration" | "constructor_declaration" => {
                if let Some(entity) = extract_function(&child, source, file_path, node)? {
                    entities.push(entity);
                }
            }
            _ => {
                collect_functions(&child, source, file_path, entities)?;
            }
        }
    }

    Ok(())
}

/// Extrae una CodeEntity de un nodo de método Java.
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

    let doc_id_line = find_docs_annotation(func_node, source, parent_node, "line_comment");
    let doc_id = if doc_id_line.is_some() {
        doc_id_line
    } else {
        find_docs_annotation(func_node, source, parent_node, "block_comment")
    };

    let line = func_node.start_position().row + 1;

    Ok(Some(CodeEntity {
        name,
        args,
        return_type,
        doc_id,
        file_path: file_path.to_path_buf(),
        line,
        is_public: true,
    }))
}

/// Extrae los parámetros de una función Java.
fn extract_parameters(func_node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Arg>> {
    let mut args = Vec::new();

    let params_node = match func_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return Ok(args),
    };

    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
         if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
             let param_name = child
                .child_by_field_name("name")
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
         }
    }

    Ok(args)
}

/// Extrae el tipo de retorno de una función Java.
fn extract_return_type(func_node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    func_node
        .child_by_field_name("type")
        .and_then(|n| n.utf8_text(source).ok())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_java_method() {
        let source = r#"
class Test {
    // @docs: [java-test]
    public boolean javaTest(int a, String b) {
        return true;
    }
}
        "#;
        let entities = parse_java_source(source, &PathBuf::from("test.java")).unwrap();
        assert_eq!(entities.len(), 1);
        let entity = &entities[0];
        assert_eq!(entity.name, "javaTest");
        assert_eq!(entity.doc_id, Some("java-test".to_string()));
        assert_eq!(entity.return_type, Some("boolean".to_string()));
        assert_eq!(entity.args.len(), 2);
        assert_eq!(entity.args[0].name, "a");
        assert_eq!(entity.args[0].type_name, Some("int".to_string()));
        assert_eq!(entity.args[1].name, "b");
        assert_eq!(entity.args[1].type_name, Some("String".to_string()));
    }
}
