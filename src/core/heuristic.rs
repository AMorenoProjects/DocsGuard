//! Heurística de matching entre funciones de código y secciones de documentación.
//!
//! Usa distancia de Levenshtein normalizada para sugerir enlaces
//! candidatos entre funciones sin `@docs` y secciones sin enlace.

use strsim::normalized_levenshtein;

use crate::core::types::{CodeEntity, DocSection};

/// Un enlace candidato sugerido por la heurística.
#[derive(Debug, Clone)]
pub struct CandidateLink {
    /// Índice de la entidad de código en el vector original.
    pub entity_index: usize,
    /// Nombre de la función.
    pub function_name: String,
    /// Ubicación en el código.
    pub code_location: String,
    /// Índice de la sección de documentación en el vector original.
    #[allow(dead_code)]
    pub section_index: usize,
    /// ID de la sección de docs.
    pub section_id: String,
    /// Título de la sección de docs.
    pub section_title: String,
    /// Confianza del match (0.0 - 1.0).
    pub confidence: f64,
}

/// Umbral mínimo de confianza para sugerir un enlace (Blueprint §3.2: >80%).
const MIN_CONFIDENCE: f64 = 0.80;

/// Genera candidatos de enlace entre funciones sin `@docs` y secciones sin enlace.
pub fn find_candidates(
    code_entities: &[CodeEntity],
    doc_sections: &[DocSection],
) -> Vec<CandidateLink> {
    let unlinked_entities: Vec<(usize, &CodeEntity)> = code_entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.doc_id.is_none())
        .collect();

    let unlinked_sections: Vec<(usize, &DocSection)> = doc_sections
        .iter()
        .enumerate()
        .filter(|&(_, s)| {
            !code_entities
                .iter()
                .any(|e| e.doc_id.as_ref() == Some(&s.id))
        })
        .collect();

    let mut candidates = Vec::new();

    for (ei, entity) in &unlinked_entities {
        let mut best_match: Option<CandidateLink> = None;

        for (si, section) in &unlinked_sections {
            let confidence = compute_confidence(&entity.name, section);

            if confidence >= MIN_CONFIDENCE {
                let candidate = CandidateLink {
                    entity_index: *ei,
                    function_name: entity.name.clone(),
                    code_location: format!("{}:{}", entity.file_path.display(), entity.line),
                    section_index: *si,
                    section_id: section.id.clone(),
                    section_title: section.title.clone().unwrap_or_else(|| section.id.clone()),
                    confidence,
                };

                if best_match
                    .as_ref()
                    .is_none_or(|b| confidence > b.confidence)
                {
                    best_match = Some(candidate);
                }
            }
        }

        if let Some(candidate) = best_match {
            candidates.push(candidate);
        }
    }

    // Ordenar por confianza descendente
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
}

/// Calcula la confianza de un match entre un nombre de función y una sección de docs.
/// Compara contra el título y el ID de la sección.
fn compute_confidence(function_name: &str, section: &DocSection) -> f64 {
    let fn_normalized = normalize_name(function_name);

    // Comparar contra el ID de la sección
    let id_normalized = normalize_name(&section.id);
    let id_similarity = normalized_levenshtein(&fn_normalized, &id_normalized);

    // Comparar contra el título si existe
    let title_similarity = section
        .title
        .as_ref()
        .map(|t| {
            let title_normalized = normalize_name(t);
            normalized_levenshtein(&fn_normalized, &title_normalized)
        })
        .unwrap_or(0.0);

    // Tomar la mayor similitud
    id_similarity.max(title_similarity)
}

/// Normaliza un nombre para comparación: lowercase, reemplaza separadores por espacios.
fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .replace(['-', '_', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entity(name: &str) -> CodeEntity {
        CodeEntity {
            name: name.into(),
            args: vec![],
            return_type: None,
            doc_id: None,
            file_path: PathBuf::from("test.rs"),
            line: 1,
        }
    }

    fn section(id: &str, title: &str) -> DocSection {
        DocSection {
            id: id.into(),
            title: Some(title.into()),
            args: vec![],
            file_path: PathBuf::from("test.md"),
            line: 1,
        }
    }

    #[test]
    fn exact_match_high_confidence() {
        let confidence = compute_confidence("login", &section("auth-login", "Login"));
        assert!(confidence > 0.7);
    }

    #[test]
    fn similar_names_match() {
        let confidence = compute_confidence("create_user", &section("user-create", "Create User"));
        assert!(confidence > 0.6);
    }

    #[test]
    fn unrelated_names_low_confidence() {
        let confidence = compute_confidence("parse_markdown", &section("auth-login", "Login"));
        assert!(confidence < MIN_CONFIDENCE);
    }

    #[test]
    fn find_candidates_returns_matches() {
        let entities = vec![entity("login"), entity("create_user")];
        let sections = vec![
            section("auth-login", "Login"),
            section("user-create", "Create User"),
        ];

        let candidates = find_candidates(&entities, &sections);
        // Al menos login debería matchear con auth-login
        assert!(!candidates.is_empty());
    }
}
