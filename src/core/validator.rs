//! Lógica de validación: compara entidades de código con secciones de documentación.
//!
//! Validaciones implementadas:
//! 1. Enlaces estáticos — ¿el doc_id del código existe en el markdown?
//! 2. Argumentos fantasma — ¿hay args en docs que no existen en código?
//! 3. Argumentos faltantes — ¿hay args en código que no están documentados?
//! 4. Type mismatch — ¿el tipo documentado coincide con el del código?

use crate::core::types::{Arg, CodeEntity, DocSection, Severity, ValidationResult};

/// @docs: [validate-links]
/// Valida que cada `CodeEntity` con un `doc_id` tenga una sección correspondiente
/// en la documentación, y compara argumentos y tipos cuando el enlace existe.
pub fn validate_links(
    code_entities: &[CodeEntity],
    doc_sections: &[DocSection],
) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    // Entidades de código sin anotación @docs
    for entity in code_entities.iter().filter(|e| e.doc_id.is_none()) {
        results.push(ValidationResult {
            severity: Severity::Info,
            message: "Función sin anotación @docs — no está vinculada a documentación.".into(),
            function_name: Some(entity.name.clone()),
            code_location: Some(format!("{}:{}", entity.file_path.display(), entity.line)),
            doc_id: None,
            hint: Some("Añade `/// @docs: [id]` antes de la función para vincularla.".into()),
        });
    }

    // Entidades de código con anotación @docs: validar enlace + argumentos
    for entity in code_entities.iter().filter(|e| e.doc_id.is_some()) {
        let doc_id = match entity.doc_id.as_ref() {
            Some(id) => id,
            None => continue, // Defensivo: no debería ocurrir tras el filtro
        };
        let location = format!("{}:{}", entity.file_path.display(), entity.line);

        let matching_section = doc_sections.iter().find(|s| &s.id == doc_id);

        match matching_section {
            Some(section) => {
                results.push(ValidationResult {
                    severity: Severity::Info,
                    message: format!(
                        "Enlace verificado: fn {} <-> sección '{}'",
                        entity.name,
                        section.title.as_deref().unwrap_or(&section.id)
                    ),
                    function_name: Some(entity.name.clone()),
                    code_location: Some(location.clone()),
                    doc_id: Some(doc_id.clone()),
                    hint: None,
                });

                // Validar argumentos si la sección tiene args documentados
                if !section.args.is_empty() || !entity.args.is_empty() {
                    validate_args(entity, section, &location, &mut results);
                }
            }
            None => {
                results.push(ValidationResult {
                    severity: Severity::Error,
                    message: format!(
                        "ID de documentación '{}' no encontrado en el archivo de docs.",
                        doc_id
                    ),
                    function_name: Some(entity.name.clone()),
                    code_location: Some(location),
                    doc_id: Some(doc_id.clone()),
                    hint: Some(format!(
                        "Añade `<!-- @docs-id: {} -->` en el archivo de documentación.",
                        doc_id
                    )),
                });
            }
        }
    }

    // Secciones de docs sin enlace desde el código
    for section in doc_sections {
        let has_link = code_entities
            .iter()
            .any(|e| e.doc_id.as_ref() == Some(&section.id));

        if !has_link {
            results.push(ValidationResult {
                severity: Severity::Warning,
                message: format!(
                    "Sección de documentación '{}' no está vinculada desde ninguna función.",
                    section.title.as_deref().unwrap_or(&section.id)
                ),
                function_name: None,
                code_location: None,
                doc_id: Some(section.id.clone()),
                hint: Some(format!(
                    "Añade `/// @docs: [{}]` antes de la función correspondiente en el código.",
                    section.id
                )),
            });
        }
    }

    results
}

/// Compara los argumentos del código con los documentados.
/// Detecta: args fantasma, args faltantes, y type mismatches.
fn validate_args(
    entity: &CodeEntity,
    section: &DocSection,
    location: &str,
    results: &mut Vec<ValidationResult>,
) {
    let doc_id = entity.doc_id.as_deref().unwrap_or("?");

    // Argumentos en docs que no existen en código (fantasma)
    for doc_arg in &section.args {
        let code_match = entity.args.iter().find(|a| a.name == doc_arg.name);

        match code_match {
            None => {
                results.push(ValidationResult {
                    severity: Severity::Error,
                    message: format!(
                        "Argumento fantasma: '{}' está documentado pero no existe en fn {}.",
                        doc_arg.name, entity.name
                    ),
                    function_name: Some(entity.name.clone()),
                    code_location: Some(location.to_string()),
                    doc_id: Some(doc_id.to_string()),
                    hint: Some(format!(
                        "Elimina '{}' de la documentación o añádelo a la firma de la función.",
                        doc_arg.name
                    )),
                });
            }
            Some(code_arg) => {
                // Verificar type mismatch si ambos tienen tipo
                check_type_mismatch(entity, code_arg, doc_arg, location, doc_id, results);
            }
        }
    }

    // Argumentos en código que no están documentados (faltantes)
    for code_arg in &entity.args {
        let is_documented = section.args.iter().any(|a| a.name == code_arg.name);

        if !is_documented {
            results.push(ValidationResult {
                severity: Severity::Warning,
                message: format!(
                    "El argumento '{}' existe en código pero falta en la documentación.",
                    code_arg.name
                ),
                function_name: Some(entity.name.clone()),
                code_location: Some(location.to_string()),
                doc_id: Some(doc_id.to_string()),
                hint: Some(format!(
                    "Documenta el argumento '{}' en la sección '{}'.",
                    code_arg.name, doc_id
                )),
            });
        }
    }
}

/// Verifica si el tipo documentado coincide con el del código.
/// Usa normalización básica para manejar alias comunes (Blueprint §4.3).
fn check_type_mismatch(
    entity: &CodeEntity,
    code_arg: &Arg,
    doc_arg: &Arg,
    location: &str,
    doc_id: &str,
    results: &mut Vec<ValidationResult>,
) {
    let code_type = match &code_arg.type_name {
        Some(t) => t,
        None => return, // Sin tipo en código, no se puede comparar
    };
    let doc_type = match &doc_arg.type_name {
        Some(t) => t,
        None => return, // Sin tipo en docs, no se puede comparar
    };

    let code_normalized = normalize_type(code_type);
    let doc_normalized = normalize_type(doc_type);

    if code_normalized != doc_normalized {
        results.push(ValidationResult {
            severity: Severity::Warning,
            message: format!(
                "Type mismatch en argumento '{}': código tiene '{}', docs dice '{}'.",
                code_arg.name, code_type, doc_type
            ),
            function_name: Some(entity.name.clone()),
            code_location: Some(location.to_string()),
            doc_id: Some(doc_id.to_string()),
            hint: Some(format!(
                "Actualiza el tipo de '{}' en la documentación a '{}' (o verifica si es un alias válido).",
                code_arg.name, code_type
            )),
        });
    }
}

/// Normaliza un tipo para comparación, manejando alias comunes.
/// Blueprint §4.3: String/str -> string, i32/u64 -> number, bool -> boolean.
fn normalize_type(type_str: &str) -> String {
    let cleaned = type_str.trim().to_lowercase();

    match cleaned.as_str() {
        // Texto
        "string" | "str" | "&str" | "text" | "&string" => "string".to_string(),
        // Números
        "number" | "integer" | "int" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8"
        | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" | "float" | "double" => {
            "number".to_string()
        }
        // Booleanos
        "boolean" | "bool" => "boolean".to_string(),
        // UUID
        "uuid" => "string".to_string(),
        // Cualquier otro tipo: comparar tal cual (normalizado a lowercase)
        _ => cleaned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_entity(name: &str, doc_id: Option<&str>) -> CodeEntity {
        CodeEntity {
            name: name.into(),
            args: vec![],
            return_type: None,
            doc_id: doc_id.map(String::from),
            file_path: PathBuf::from("test.ts"),
            line: 1,
        }
    }

    fn make_entity_with_args(name: &str, doc_id: &str, args: Vec<Arg>) -> CodeEntity {
        CodeEntity {
            name: name.into(),
            args,
            return_type: None,
            doc_id: Some(doc_id.into()),
            file_path: PathBuf::from("test.ts"),
            line: 1,
        }
    }

    fn make_section(id: &str, title: Option<&str>) -> DocSection {
        DocSection {
            id: id.into(),
            title: title.map(String::from),
            args: vec![],
            file_path: PathBuf::from("test.md"),
            line: 1,
        }
    }

    fn make_section_with_args(id: &str, title: &str, args: Vec<Arg>) -> DocSection {
        DocSection {
            id: id.into(),
            title: Some(title.into()),
            args,
            file_path: PathBuf::from("test.md"),
            line: 1,
        }
    }

    fn arg(name: &str, type_name: Option<&str>) -> Arg {
        Arg {
            name: name.into(),
            type_name: type_name.map(String::from),
            description: None,
        }
    }

    #[test]
    fn matching_link_produces_info() {
        let entities = vec![make_entity("login", Some("auth-login"))];
        let sections = vec![make_section("auth-login", Some("Login"))];
        let results = validate_links(&entities, &sections);

        let infos: Vec<_> = results
            .iter()
            .filter(|r| r.severity == Severity::Info)
            .collect();
        assert_eq!(infos.len(), 1);
        assert!(infos[0].message.contains("Enlace verificado"));
    }

    #[test]
    fn missing_doc_section_produces_error() {
        let entities = vec![make_entity("login", Some("auth-login"))];
        let sections = vec![];
        let results = validate_links(&entities, &sections);

        let errors: Vec<_> = results
            .iter()
            .filter(|r| r.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("no encontrado"));
    }

    #[test]
    fn orphan_doc_section_produces_warning() {
        let entities = vec![];
        let sections = vec![make_section("auth-login", Some("Login"))];
        let results = validate_links(&entities, &sections);

        let warnings: Vec<_> = results
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("no está vinculada"));
    }

    #[test]
    fn ghost_arg_produces_error() {
        let entities = vec![make_entity_with_args(
            "login",
            "auth-login",
            vec![arg("username", Some("string"))],
        )];
        let sections = vec![make_section_with_args(
            "auth-login",
            "Login",
            vec![
                arg("username", Some("string")),
                arg("tenant_id", Some("string")), // No existe en código
            ],
        )];

        let results = validate_links(&entities, &sections);
        let errors: Vec<_> = results
            .iter()
            .filter(|r| r.severity == Severity::Error && r.message.contains("fantasma"))
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("tenant_id"));
    }

    #[test]
    fn missing_arg_in_docs_produces_warning() {
        let entities = vec![make_entity_with_args(
            "login",
            "auth-login",
            vec![
                arg("username", Some("string")),
                arg("password", Some("string")),
            ],
        )];
        let sections = vec![make_section_with_args(
            "auth-login",
            "Login",
            vec![arg("username", Some("string"))],
            // password falta en docs
        )];

        let results = validate_links(&entities, &sections);
        let warnings: Vec<_> = results
            .iter()
            .filter(|r| r.severity == Severity::Warning && r.message.contains("falta"))
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("password"));
    }

    #[test]
    fn type_mismatch_produces_warning() {
        let entities = vec![make_entity_with_args(
            "login",
            "auth-login",
            vec![arg("tenant_id", Some("string"))], // código dice string
        )];
        let sections = vec![make_section_with_args(
            "auth-login",
            "Login",
            vec![arg("tenant_id", Some("Integer"))], // docs dice Integer
        )];

        let results = validate_links(&entities, &sections);
        let mismatches: Vec<_> = results
            .iter()
            .filter(|r| r.message.contains("Type mismatch"))
            .collect();
        assert_eq!(mismatches.len(), 1);
        assert!(mismatches[0].message.contains("tenant_id"));
    }

    #[test]
    fn type_alias_matches_correctly() {
        // "str" y "String" deben normalizar al mismo tipo
        let entities = vec![make_entity_with_args(
            "greet",
            "greet-fn",
            vec![arg("name", Some("&str"))],
        )];
        let sections = vec![make_section_with_args(
            "greet-fn",
            "Greet",
            vec![arg("name", Some("String"))],
        )];

        let results = validate_links(&entities, &sections);
        let mismatches: Vec<_> = results
            .iter()
            .filter(|r| r.message.contains("Type mismatch"))
            .collect();
        assert_eq!(mismatches.len(), 0); // No debe haber mismatch
    }

    #[test]
    fn normalize_type_aliases() {
        assert_eq!(normalize_type("String"), "string");
        assert_eq!(normalize_type("&str"), "string");
        assert_eq!(normalize_type("i32"), "number");
        assert_eq!(normalize_type("Integer"), "number");
        assert_eq!(normalize_type("bool"), "boolean");
        assert_eq!(normalize_type("Boolean"), "boolean");
        assert_eq!(normalize_type("UUID"), "string");
        assert_eq!(normalize_type("CustomType"), "customtype");
    }
}
