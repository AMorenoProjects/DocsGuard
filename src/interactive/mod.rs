//! Scaffold TUI interactivo (Blueprint §4.1).
//!
//! Muestra sugerencias de enlaces código-doc una por una.
//! Nunca toca el disco sin permiso explícito.

use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use std::path::Path;

use crate::core::heuristic::{self, CandidateLink};
use crate::core::types::CodeEntity;
use crate::parser::{code_parser, doc_parser};

/// Resultado de la decisión del usuario sobre un candidato.
#[derive(Debug)]
enum UserDecision {
    Accept,
    Reject,
    Skip,
}

/// Ejecuta el scaffold interactivo.
/// Parsea código y docs, encuentra candidatos heurísticos, y presenta
/// cada sugerencia al usuario para confirmación.
pub fn run_scaffold(code_file: &Path, doc_file: &Path, dry_run: bool, force: bool) -> Result<()> {
    if !code_file.exists() {
        anyhow::bail!(
            "Archivo de código no encontrado: {}\n    -> Verifica que la ruta sea correcta.",
            code_file.display()
        );
    }
    if !doc_file.exists() {
        anyhow::bail!(
            "Archivo de documentación no encontrado: {}\n    -> Verifica que la ruta sea correcta.",
            doc_file.display()
        );
    }

    println!("DocsGuard Scaffold — Vinculación interactiva código ↔ documentación\n");

    if dry_run {
        println!("  [modo dry-run] No se escribirán cambios al disco.\n");
    }

    let code_entities =
        code_parser::parse_code_file(code_file).context("Error al parsear el archivo de código")?;

    let doc_sections = doc_parser::parse_markdown_file(doc_file)
        .context("Error al parsear el archivo de documentación")?;

    let candidates = heuristic::find_candidates(&code_entities, &doc_sections);

    if candidates.is_empty() {
        println!("  No se encontraron sugerencias de enlace.");
        println!("  (Todas las funciones ya están vinculadas o no hay matches heurísticos)");
        return Ok(());
    }

    println!(
        "  Encontradas {} sugerencias de enlace (confianza ≥ 80%).\n",
        candidates.len()
    );

    let mut accepted: Vec<&CandidateLink> = Vec::new();
    let mut rejected = 0;

    for (i, candidate) in candidates.iter().enumerate() {
        println!(
            "── Sugerencia {}/{} ──────────────────────────────",
            i + 1,
            candidates.len()
        );
        println!(
            "  Función:  {} ({})",
            candidate.function_name, candidate.code_location
        );
        println!(
            "  Sección:  '{}' [id: {}]",
            candidate.section_title, candidate.section_id
        );
        println!("  Confianza: {:.0}%", candidate.confidence * 100.0);
        println!();

        let decision = if force {
            UserDecision::Accept
        } else {
            prompt_user()?
        };

        match decision {
            UserDecision::Accept => {
                accepted.push(candidate);
                println!("  → Aceptado.\n");
            }
            UserDecision::Reject => {
                rejected += 1;
                println!("  → Rechazado.\n");
            }
            UserDecision::Skip => {
                println!("  → Omitido.\n");
            }
        }
    }

    println!("── Resumen ──────────────────────────────────────");
    println!("  Aceptados: {}", accepted.len());
    println!("  Rechazados: {}", rejected);
    println!(
        "  Omitidos: {}",
        candidates.len() - accepted.len() - rejected
    );

    if accepted.is_empty() {
        println!("\n  No hay cambios que aplicar.");
        return Ok(());
    }

    if dry_run {
        println!("\n  [dry-run] Cambios que se habrían escrito:");
        for candidate in &accepted {
            println!(
                "    • {} → /// @docs: [{}]",
                candidate.function_name, candidate.section_id
            );
        }
        println!("\n  Ejecuta sin --dry-run para aplicar los cambios.");
    } else {
        apply_changes(code_file, &code_entities, &accepted)?;
        println!(
            "\n  {} enlaces escritos en {}.",
            accepted.len(),
            code_file.display()
        );
    }

    Ok(())
}

/// Presenta la prompt interactiva al usuario.
fn prompt_user() -> Result<UserDecision> {
    let selections = &["Sí — vincular", "No — rechazar", "Omitir"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("¿Vincular esta función con esta sección?")
        .items(selections)
        .default(0)
        .interact()
        .context("Error al leer la respuesta del usuario")?;

    Ok(match selection {
        0 => UserDecision::Accept,
        1 => UserDecision::Reject,
        _ => UserDecision::Skip,
    })
}

/// Aplica los cambios aceptados al archivo de código.
/// Inserta `/// @docs: [id]` antes de cada función vinculada.
fn apply_changes(
    code_file: &Path,
    code_entities: &[CodeEntity],
    accepted: &[&CandidateLink],
) -> Result<()> {
    let source = std::fs::read_to_string(code_file)
        .with_context(|| format!("No se pudo leer: {}", code_file.display()))?;

    let lines: Vec<&str> = source.lines().collect();
    let mut output_lines: Vec<String> = Vec::with_capacity(lines.len() + accepted.len());

    // Construir mapa de línea → anotación a insertar (0-indexed)
    let mut annotations: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();
    for candidate in accepted {
        let entity = code_entities.get(candidate.entity_index).with_context(|| {
            format!(
                "Índice de entidad inválido: {} (total: {})",
                candidate.entity_index,
                code_entities.len()
            )
        })?;
        let line_0indexed = entity.line.saturating_sub(1);
        annotations.insert(
            line_0indexed,
            format!("/// @docs: [{}]", candidate.section_id),
        );
    }

    for (i, line) in lines.iter().enumerate() {
        if let Some(annotation) = annotations.get(&i) {
            // Detectar indentación de la línea de la función
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            output_lines.push(format!("{}{}", indent, annotation));
        }
        output_lines.push(line.to_string());
    }

    let mut result = output_lines.join("\n");
    // Preservar newline final si el original lo tenía
    if source.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(code_file, result)
        .with_context(|| format!("No se pudo escribir: {}", code_file.display()))?;

    Ok(())
}
