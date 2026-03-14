//! Parser para Python usando tree-sitter.
//!
//! Extrae funciones de archivos Python y busca anotaciones `# @docs: [id]`
//! en los comentarios inmediatamente anteriores a la declaración.

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

use crate::core::types::{Arg, CodeEntity};
use crate::parser::code_parser::find_docs_annotation;

/// Parsea código Python desde un string.
pub fn parse_python_source(source: &str, file_path: &Path) -> Result<Vec<CodeEntity>> {
    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser
        .set_language(&language.into())
        .context("Error al configurar tree-sitter con Python")?;

    let tree = parser
        .parse(source, None)
        .context("Error al parsear el archivo Python")?;

    let root_node = tree.root_node();
    let source_bytes = source.as_bytes();
    let mut entities = Vec::new();

    collect_functions(&root_node, source_bytes, file_path, &mut entities)?;

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
                         if let Some(entity) = extract_function(&definition, source, file_path, node)? {
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

/// Extrae los parámetros de una función Python.
fn extract_parameters(func_node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Arg>> {
    let mut args = Vec::new();

    let params_node = match func_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return Ok(args),
    };

    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let param_name = child.utf8_text(source).unwrap_or("").to_string();
            if param_name != "self" && param_name != "cls" {
                args.push(Arg {
                    name: param_name,
                    type_name: None,
                    description: None,
                });
            }
        } else if child.kind() == "typed_parameter" {
            // El `typed_parameter` no tiene fields `name` o `type`,
            // sino que sus hijos anónimos proveen esta información.
            // Para Python, el primer hijo usualmente es el identifier, y hay un `type` child.
            let mut param_name = String::new();
            let mut type_name = None;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    param_name = inner_child.utf8_text(source).unwrap_or("").to_string();
                } else if inner_child.kind() == "type" {
                    type_name = inner_child.utf8_text(source).ok().map(String::from);
                }
            }

            if param_name != "self" && param_name != "cls" && !param_name.is_empty() {
                args.push(Arg {
                    name: param_name,
                    type_name,
                    description: None,
                });
            }
        } else if child.kind() == "default_parameter" || child.kind() == "typed_default_parameter" {
             let name_node = child.child_by_field_name("name");
             if let Some(name_n) = name_node {
                  if name_n.kind() == "identifier" {
                       let param_name = name_n.utf8_text(source).unwrap_or("").to_string();
                        if param_name != "self" && param_name != "cls" && !param_name.is_empty() {
                             args.push(Arg {
                                  name: param_name,
                                  type_name: None,
                                  description: None,
                             });
                        }
                  } else if name_n.kind() == "typed_parameter" {
                       let mut param_name = String::new();
                       let mut type_name = None;

                       let mut inner_cursor = name_n.walk();
                       for inner_child in name_n.children(&mut inner_cursor) {
                            if inner_child.kind() == "identifier" {
                                 param_name = inner_child.utf8_text(source).unwrap_or("").to_string();
                            } else if inner_child.kind() == "type" {
                                 type_name = inner_child.utf8_text(source).ok().map(String::from);
                            }
                       }

                       if param_name != "self" && param_name != "cls" && !param_name.is_empty() {
                            args.push(Arg {
                                 name: param_name,
                                 type_name,
                                 description: None,
                            });
                       }
                  }
             }
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
    fn dump_python_ast() {
        let source = r#"
# @docs: [python-test]
def python_test(a: int, b: str) -> bool:
    return True
"#;
        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE;
        parser
            .set_language(&language.into())
            .unwrap();

        let tree = parser
            .parse(source, None)
            .unwrap();

        println!("{}", tree.root_node().to_sexp());
        
        // Also let's run the parser:
        let entities = parse_python_source(source, &PathBuf::from("test.py")).unwrap();
        println!("{:#?}", entities);
    }
}
