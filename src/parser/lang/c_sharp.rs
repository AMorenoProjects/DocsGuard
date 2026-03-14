//! Parser para C# usando tree-sitter.
//!
//! Extrae funciones de archivos C# y busca anotaciones `// @docs: [id]`
//! en los comentarios inmediatamente anteriores a la declaración.

use anyhow::Result;
use std::path::Path;

use crate::core::types::{Arg, CodeEntity};
use crate::parser::code_parser;
use crate::parser::code_parser::find_docs_annotation;

/// Parsea código C# desde un string.
pub fn parse_c_sharp_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>> {
    // Refactorizado: uso de create_tree para eliminar boilerplate duplicado entre parsers
    let tree = code_parser::create_tree(source, tree_sitter_c_sharp::LANGUAGE.into(), "C#")?;
    let mut entities = Vec::new();
    collect_functions(&tree.root_node(), source.as_bytes(), file_path, &mut entities)?;
    Ok(entities)
}

/// Recorre el AST recursivamente buscando constructores, métodos y funciones locales.
fn collect_functions(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    entities: &mut Vec<CodeEntity>,
) -> Result<()> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "method_declaration" | "constructor_declaration" | "local_function_statement" => {
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

/// Extrae una CodeEntity de un nodo de función C#.
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

    let doc_id = find_docs_annotation(func_node, source, parent_node, "comment");

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

/// Extrae los parámetros de una función C#.
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

/// Extrae el tipo de retorno de una función C#.
fn extract_return_type(func_node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    func_node
        .child_by_field_name("returns")
        .and_then(|n| n.utf8_text(source).ok())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_csharp_method() {
        let source = r#"
class Test {
    // @docs: [csharp-test]
    public bool CSharpTest(int a, string b) {
        return true;
    }
}
        "#;
        let entities = parse_c_sharp_source(source, &PathBuf::from("test.cs")).unwrap();
        assert_eq!(entities.len(), 1);
        let entity = &entities[0];
        assert_eq!(entity.name, "CSharpTest");
        assert_eq!(entity.doc_id, Some("csharp-test".to_string()));
        assert_eq!(entity.return_type, Some("bool".to_string()));
        assert_eq!(entity.args.len(), 2);
        assert_eq!(entity.args[0].name, "a");
        assert_eq!(entity.args[0].type_name, Some("int".to_string()));
        assert_eq!(entity.args[1].name, "b");
        assert_eq!(entity.args[1].type_name, Some("string".to_string()));
    }
}
