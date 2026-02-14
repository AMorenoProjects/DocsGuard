//! Parser de TypeScript usando tree-sitter.

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

use crate::core::types::{Arg, CodeEntity};
use crate::parser::code_parser::find_docs_annotation;

/// Parsea código TypeScript desde un string.
pub fn parse_typescript_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>> {
    let mut parser = Parser::new();
    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
    parser
        .set_language(&language.into())
        .context("Error al configurar tree-sitter con TypeScript")?;

    let tree = parser
        .parse(source, None)
        .context("Error al parsear el archivo TypeScript")?;

    let root_node = tree.root_node();
    let source_bytes = source.as_bytes();
    let mut entities = Vec::new();

    collect_functions(&root_node, source_bytes, file_path, &mut entities)?;

    Ok(entities)
}

fn collect_functions(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    entities: &mut Vec<CodeEntity>,
) -> Result<()> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "export_statement" => {
                let func_node = if child.kind() == "export_statement" {
                    find_function_in_export(&child)
                } else {
                    Some(child)
                };

                if let Some(func_node) = func_node {
                    if let Some(entity) = extract_function(&func_node, source, file_path, node)? {
                        entities.push(entity);
                    }
                }
            }
            _ => {
                collect_functions(&child, source, file_path, entities)?;
            }
        }
    }

    Ok(())
}

fn find_function_in_export<'a>(
    export_node: &tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = export_node.walk();
    let result = export_node
        .children(&mut cursor)
        .find(|child| child.kind() == "function_declaration");
    result
}

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
    }))
}

fn extract_parameters(func_node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Arg>> {
    let mut args = Vec::new();

    let params_node = match func_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return Ok(args),
    };

    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if child.kind() == "required_parameter" || child.kind() == "optional_parameter" {
            let param_name = child
                .child_by_field_name("pattern")
                .and_then(|n| n.utf8_text(source).ok())
                .map(String::from)
                .unwrap_or_default();

            let type_name = child
                .child_by_field_name("type")
                .and_then(|type_ann| {
                    let mut tc = type_ann.walk();
                    let result = type_ann
                        .children(&mut tc)
                        .find(|c| c.kind() != ":")
                        .and_then(|c| c.utf8_text(source).ok());
                    result
                })
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

fn extract_return_type(func_node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    func_node
        .child_by_field_name("return_type")
        .and_then(|type_ann| {
            let mut cursor = type_ann.walk();
            let result = type_ann
                .children(&mut cursor)
                .find(|c| c.kind() != ":")
                .and_then(|c| c.utf8_text(source).ok());
            result
        })
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_simple_function() {
        let source = r#"
/// @docs: [auth-login]
function login(username: string, password: string): boolean {
    return true;
}
"#;
        let entities = parse_typescript_source(source, &PathBuf::from("test.ts")).unwrap();
        assert_eq!(entities.len(), 1);

        let entity = &entities[0];
        assert_eq!(entity.name, "login");
        assert_eq!(entity.doc_id, Some("auth-login".into()));
        assert_eq!(entity.args.len(), 2);
        assert_eq!(entity.args[0].name, "username");
        assert_eq!(entity.args[0].type_name.as_deref(), Some("string"));
        assert_eq!(entity.args[1].name, "password");
    }

    #[test]
    fn parse_function_without_annotation() {
        let source = r#"
function helper(): void {
}
"#;
        let entities = parse_typescript_source(source, &PathBuf::from("test.ts")).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "helper");
        assert_eq!(entities[0].doc_id, None);
    }

    #[test]
    fn parse_exported_function() {
        let source = r#"
/// @docs: [user-create]
export function createUser(name: string): User {
    return new User(name);
}
"#;
        let entities = parse_typescript_source(source, &PathBuf::from("test.ts")).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "createUser");
        assert_eq!(entities[0].doc_id, Some("user-create".into()));
    }
}
