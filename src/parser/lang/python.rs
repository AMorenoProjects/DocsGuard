//! Parser para Python usando tree-sitter.
//!
//! Extrae funciones de archivos Python y busca anotaciones `# @docs: [id]`
//! en los comentarios inmediatamente anteriores a la declaración.

use anyhow::Result;
use std::path::Path;

use crate::core::types::{Arg, CodeEntity};
use crate::parser::code_parser;
use crate::parser::code_parser::find_docs_annotation;

/// Parsea código Python desde un string.
pub fn parse_python_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>> {
    // Refactorizado: uso de create_tree para eliminar boilerplate duplicado entre parsers
    let tree = code_parser::create_tree(source, tree_sitter_python::LANGUAGE.into(), "Python")?;
    let mut entities = Vec::new();
    collect_functions(
        &tree.root_node(),
        source.as_bytes(),
        file_path,
        &mut entities,
    )?;
    Ok(entities)
}

/// Recorre el AST recursivamente buscando `function_definition` nodes.
fn collect_functions(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    entities: &mut Vec<CodeEntity>,
) -> Result<()> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(entity) = extract_function(&child, source, file_path, node)? {
                    entities.push(entity);
                }
            }
            "class_definition" => {
                let body = child.child_by_field_name("body");
                if let Some(body) = body {
                    collect_functions(&body, source, file_path, entities)?;
                }
            }
            "decorated_definition" => {
                if let Some(definition) = child.child_by_field_name("definition") {
                    if definition.kind() == "function_definition" {
                        if let Some(entity) =
                            extract_function(&definition, source, file_path, node)?
                        {
                            entities.push(entity);
                        }
                    } else if definition.kind() == "class_definition" {
                        let body = definition.child_by_field_name("body");
                        if let Some(body) = body {
                            collect_functions(&body, source, file_path, entities)?;
                        }
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

/// Extrae una CodeEntity de un nodo `function_definition` de Python.
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

    // En Python, los comentarios `#` son nodos `comment` en tree-sitter.
    // Hay que buscarlos como hermanos del `function_definition` o del `decorated_definition`
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

/// Extrae nombre y tipo de un nodo `typed_parameter` de Python.
///
/// Refactorizado: función DRY que elimina la duplicación de lógica idéntica
/// en los arms `typed_parameter` y `typed_default_parameter`.
fn extract_typed_param_info(node: &tree_sitter::Node, source: &[u8]) -> (String, Option<String>) {
    let mut param_name = String::new();
    let mut type_name = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            param_name = child.utf8_text(source).unwrap_or("").to_string();
        } else if child.kind() == "type" {
            type_name = child.utf8_text(source).ok().map(String::from);
        }
    }
    (param_name, type_name)
}

/// Extrae los parámetros de una función Python.
fn extract_parameters(func_node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Arg>> {
    let mut args = Vec::new();

    let params_node = match func_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return Ok(args),
    };

    let is_self_or_cls = |name: &str| name == "self" || name == "cls";

    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let param_name = child.utf8_text(source).unwrap_or("").to_string();
                if !is_self_or_cls(&param_name) {
                    args.push(Arg {
                        name: param_name,
                        type_name: None,
                        description: None,
                    });
                }
            }
            "typed_parameter" => {
                // Refactorizado: usa extract_typed_param_info en lugar de duplicar la lógica
                let (param_name, type_name) = extract_typed_param_info(&child, source);
                if !is_self_or_cls(&param_name) && !param_name.is_empty() {
                    args.push(Arg {
                        name: param_name,
                        type_name,
                        description: None,
                    });
                }
            }
            "default_parameter" | "typed_default_parameter" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    if name_n.kind() == "identifier" {
                        let param_name = name_n.utf8_text(source).unwrap_or("").to_string();
                        if !is_self_or_cls(&param_name) && !param_name.is_empty() {
                            args.push(Arg {
                                name: param_name,
                                type_name: None,
                                description: None,
                            });
                        }
                    } else if name_n.kind() == "typed_parameter" {
                        // Refactorizado: usa extract_typed_param_info en lugar de duplicar la lógica
                        let (param_name, type_name) = extract_typed_param_info(&name_n, source);
                        if !is_self_or_cls(&param_name) && !param_name.is_empty() {
                            args.push(Arg {
                                name: param_name,
                                type_name,
                                description: None,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(args)
}

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
    fn parse_python_function_with_annotation() {
        let source = r#"
# @docs: [python-test]
def python_test(a: int, b: str) -> bool:
    return True
"#;
        let entities = parse_python_source(source, &PathBuf::from("test.py")).unwrap();
        assert_eq!(entities.len(), 1);
        let entity = &entities[0];
        assert_eq!(entity.name, "python_test");
        assert_eq!(entity.doc_id, Some("python-test".to_string()));
        assert_eq!(entity.return_type.as_deref(), Some("bool"));
        assert_eq!(entity.args.len(), 2);
        assert_eq!(entity.args[0].name, "a");
        assert_eq!(entity.args[1].name, "b");
    }
}
