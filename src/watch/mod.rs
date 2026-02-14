//! Watch Mode — feedback en tiempo real (Blueprint §2.1 US-2).
//!
//! Observa cambios en archivos de código y documentación.
//! Re-ejecuta la validación y muestra resultados en terminal limpia (<200ms target).

use anyhow::{Context, Result};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::core::types::Severity;
use crate::core::validator;
use crate::parser::{code_parser, doc_parser};

/// Ejecuta el modo watch: observa cambios y re-valida automáticamente.
pub fn run_watch(code_file: &Path, doc_file: &Path) -> Result<()> {
    if !code_file.exists() {
        anyhow::bail!("Archivo de código no encontrado: {}", code_file.display());
    }
    if !doc_file.exists() {
        anyhow::bail!(
            "Archivo de documentación no encontrado: {}",
            doc_file.display()
        );
    }

    let code_file = std::fs::canonicalize(code_file)
        .with_context(|| format!("No se pudo resolver la ruta: {}", code_file.display()))?;
    let doc_file = std::fs::canonicalize(doc_file)
        .with_context(|| format!("No se pudo resolver la ruta: {}", doc_file.display()))?;

    // Validación inicial
    clear_and_validate(&code_file, &doc_file)?;

    println!("\n  Observando cambios... (Ctrl+C para salir)");

    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(150), tx)
        .context("Error al inicializar el watcher de archivos")?;

    // Observar los directorios padre de ambos archivos
    let watch_paths = collect_watch_paths(&code_file, &doc_file);
    for path in &watch_paths {
        debouncer
            .watcher()
            .watch(path, notify::RecursiveMode::NonRecursive)
            .with_context(|| format!("Error al observar: {}", path.display()))?;
    }

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                let relevant = events.iter().any(|e| {
                    e.kind == DebouncedEventKind::Any && (e.path == code_file || e.path == doc_file)
                });

                if relevant {
                    if !code_file.exists() {
                        eprintln!("  [!] Archivo de código eliminado: {}", code_file.display());
                        continue;
                    }
                    if !doc_file.exists() {
                        eprintln!(
                            "  [!] Archivo de documentación eliminado: {}",
                            doc_file.display()
                        );
                        continue;
                    }
                    clear_and_validate(&code_file, &doc_file)?;
                    println!("\n  Observando cambios... (Ctrl+C para salir)");
                }
            }
            Ok(Err(errs)) => {
                eprintln!("  [watch] Errores del watcher: {:?}", errs);
            }
            Err(_) => {
                // Canal cerrado, salir
                break;
            }
        }
    }

    Ok(())
}

/// Limpia la terminal y ejecuta la validación.
fn clear_and_validate(code_file: &Path, doc_file: &Path) -> Result<()> {
    // Limpiar pantalla
    print!("\x1B[2J\x1B[1;1H");

    let start = Instant::now();

    println!("DocsGuard Watch — Validación en tiempo real\n");
    println!("  Código: {}", code_file.display());
    println!("  Docs:   {}\n", doc_file.display());

    let code_entities = match code_parser::parse_code_file(code_file) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("  [!] Error al parsear código: {}", e);
            return Ok(());
        }
    };

    let doc_sections = match doc_parser::parse_markdown_file(doc_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  [!] Error al parsear docs: {}", e);
            return Ok(());
        }
    };

    let results = validator::validate_links(&code_entities, &doc_sections);

    let error_count = results
        .iter()
        .filter(|r| r.severity == Severity::Error)
        .count();
    let warning_count = results
        .iter()
        .filter(|r| r.severity == Severity::Warning)
        .count();

    // Mostrar solo errores y warnings (no info) en watch mode
    for result in results.iter().filter(|r| r.severity != Severity::Info) {
        print!("{result}");
    }

    let elapsed = start.elapsed();

    if error_count == 0 && warning_count == 0 {
        println!("  ✓ Sin errores ni advertencias.");
    }

    println!(
        "\n  Resumen: {} errores, {} advertencias ({}ms)",
        error_count,
        warning_count,
        elapsed.as_millis()
    );

    Ok(())
}

/// Obtiene los directorios a observar.
fn collect_watch_paths(code_file: &Path, doc_file: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(parent) = code_file.parent() {
        paths.push(parent.to_path_buf());
    }

    if let Some(parent) = doc_file.parent() {
        if !paths.contains(&parent.to_path_buf()) {
            paths.push(parent.to_path_buf());
        }
    }

    paths
}
