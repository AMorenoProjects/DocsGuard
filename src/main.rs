//! DocsGuard — Motor de Integridad de Documentación.
//!
//! Elimina la deriva código-doc mediante validación heurística,
//! soporte multiformato y corrección interactiva.

mod baseline;
mod core;
mod coverage;
mod interactive;
mod parser;
mod watch;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use crate::core::types::Severity;
use crate::core::validator;
use crate::parser::code_parser::{self, safe_display};
use crate::parser::doc_parser;

#[derive(Parser)]
#[command(
    name = "docsguard",
    version,
    about = "Motor de Integridad de Documentación — elimina la deriva código-doc"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verifica que los enlaces entre código y documentación sean válidos.
    Check {
        /// Archivo de documentación (Markdown).
        doc_file: PathBuf,
        /// Archivos de código fuente (TypeScript, Rust).
        #[arg(required = true)]
        code_files: Vec<PathBuf>,
        /// Directorio raíz del proyecto (para buscar baseline).
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },

    /// Scaffold interactivo: sugiere enlaces código ↔ docs con confirmación.
    Scaffold {
        /// Archivo de código fuente.
        code_file: PathBuf,
        /// Archivo de documentación.
        doc_file: PathBuf,
        /// No escribir cambios al disco (solo mostrar sugerencias).
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Aceptar todas las sugerencias sin preguntar.
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Observa cambios en archivos y re-valida automáticamente.
    Watch {
        /// Archivo de código fuente.
        code_file: PathBuf,
        /// Archivo de documentación.
        doc_file: PathBuf,
    },

    /// Vuelca los errores actuales al baseline para "Green Build Day 1".
    Baseline {
        /// Archivo de código fuente.
        code_file: PathBuf,
        /// Archivo de documentación.
        doc_file: PathBuf,
        /// Directorio raíz del proyecto.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },

    /// Muestra el porcentaje de funciones públicas con anotación @docs.
    Coverage {
        /// Archivos de código fuente a analizar.
        #[arg(required = true)]
        code_files: Vec<PathBuf>,
        /// Cobertura mínima requerida (0-100). Sale con código 1 si no se alcanza.
        #[arg(long, default_value_t = 80)]
        min_coverage: u8,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            code_files,
            doc_file,
            project_root,
        } => run_check(&code_files, &doc_file, &project_root),

        Commands::Scaffold {
            code_file,
            doc_file,
            dry_run,
            force,
        } => interactive::run_scaffold(&code_file, &doc_file, dry_run, force),

        Commands::Watch {
            code_file,
            doc_file,
        } => watch::run_watch(&code_file, &doc_file),

        Commands::Baseline {
            code_file,
            doc_file,
            project_root,
        } => baseline::run_baseline(&code_file, &doc_file, &project_root),

        Commands::Coverage {
            code_files,
            min_coverage,
        } => coverage::run_coverage(&code_files, min_coverage),
    }
}

fn run_check(code_files: &[PathBuf], doc_file: &Path, project_root: &Path) -> Result<()> {
    // Refactorizado: usa require_file_exists para eliminar comprobaciones duplicadas entre comandos
    for code_file in code_files {
        code_parser::require_file_exists(code_file, "código")?;
    }
    code_parser::require_file_exists(doc_file, "documentación")?;

    println!("DocsGuard — Verificando enlaces código ↔ documentación\n");
    println!("  Docs: {}", safe_display(doc_file));
    println!("  Código: {} archivos", code_files.len());

    let mut all_code_entities = Vec::new();
    for code_file in code_files {
        println!("    -> {}", safe_display(code_file));
        let mut entities = code_parser::parse_code_file(code_file)
            .context(format!("Error al parsear {}", code_file.display()))?;
        all_code_entities.append(&mut entities);
    }
    println!(); // spacer

    let doc_sections = doc_parser::parse_markdown_file(doc_file)
        .context("Error al parsear el archivo de documentación")?;

    println!(
        "  Encontradas {} funciones en código (total), {} secciones en docs.\n",
        all_code_entities.len(),
        doc_sections.len()
    );

    let results = validator::validate_links(&all_code_entities, &doc_sections);

    // Aplicar baseline si existe
    let (results, baseline_filtered) = match baseline::Baseline::load(project_root)? {
        Some(bl) => {
            let (filtered_results, count) = baseline::filter_baseline(&results, &bl);
            if count > 0 {
                println!(
                    "  [baseline] {} errores/advertencias conocidos filtrados.\n",
                    count
                );
            }
            (filtered_results, count)
        }
        None => (results, 0),
    };

    if results.is_empty() {
        if baseline_filtered > 0 {
            println!("  Sin errores nuevos (baseline activo).");
        } else {
            println!("  No se encontraron funciones ni secciones para validar.");
        }
        return Ok(());
    }

    let error_count = results
        .iter()
        .filter(|r| r.severity == Severity::Error)
        .count();
    let warning_count = results
        .iter()
        .filter(|r| r.severity == Severity::Warning)
        .count();

    for result in &results {
        print!("{result}");
    }

    println!("---");
    println!(
        "Resumen: {} errores, {} advertencias, {} total",
        error_count,
        warning_count,
        results.len()
    );

    if error_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}
