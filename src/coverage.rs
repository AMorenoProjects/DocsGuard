//! Módulo de Document Coverage Report.
//!
//! Analiza qué porcentaje de las funciones/métodos públicos exportados
//! tiene una anotación `@docs` vinculada a su sección de documentación.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::parser::code_parser;

// ── ANSI colors ────────────────────────────────────────────────────────────────
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

const BAR_WIDTH: usize = 24;
const BAR_FILLED: char = '█';
const BAR_EMPTY: char = '░';

// ── Tipos ──────────────────────────────────────────────────────────────────────

/// Resultado de cobertura de un único archivo.
pub struct FileCoverage {
    pub file: PathBuf,
    pub total_public: usize,
    pub documented: usize,
}

impl FileCoverage {
    pub fn percentage(&self) -> f64 {
        if self.total_public == 0 {
            return 100.0;
        }
        (self.documented as f64 / self.total_public as f64) * 100.0
    }
}

/// Resultado agregado de todos los archivos analizados.
pub struct CoverageReport {
    pub files: Vec<FileCoverage>,
    pub total_public: usize,
    pub total_documented: usize,
}

impl CoverageReport {
    pub fn percentage(&self) -> f64 {
        if self.total_public == 0 {
            return 100.0;
        }
        (self.total_documented as f64 / self.total_public as f64) * 100.0
    }
}

// ── Lógica principal ───────────────────────────────────────────────────────────

/// Ejecuta el análisis de cobertura de documentación.
///
/// Sale con código 1 si la cobertura total está por debajo de `min_coverage`.
pub fn run_coverage(code_files: &[PathBuf], min_coverage: u8) -> Result<()> {
    // Refactorizado: usa require_file_exists para eliminar comprobaciones duplicadas entre comandos
    for file in code_files {
        code_parser::require_file_exists(file, "código")?;
    }

    let report = build_report(code_files)?;
    print_report(&report, min_coverage);

    let pct = report.percentage();
    if pct < f64::from(min_coverage) {
        std::process::exit(1);
    }

    Ok(())
}

fn build_report(code_files: &[PathBuf]) -> Result<CoverageReport> {
    let mut file_coverages = Vec::new();
    let mut total_public = 0;
    let mut total_documented = 0;

    for file in code_files {
        let entities = code_parser::parse_code_file(file)
            .with_context(|| format!("Error al parsear {}", file.display()))?;

        let public: Vec<_> = entities.iter().filter(|e| e.is_public).collect();
        let documented = public.iter().filter(|e| e.doc_id.is_some()).count();

        total_public += public.len();
        total_documented += documented;

        file_coverages.push(FileCoverage {
            file: file.clone(),
            total_public: public.len(),
            documented,
        });
    }

    Ok(CoverageReport {
        files: file_coverages,
        total_public,
        total_documented,
    })
}

// ── Presentación ──────────────────────────────────────────────────────────────

fn print_report(report: &CoverageReport, min_coverage: u8) {
    let separator = format!("{DIM}{}{RESET}", "─".repeat(60));

    println!("\n{BOLD}{CYAN}DocsGuard — Document Coverage Report{RESET}\n");
    println!("{separator}");

    // Calcular ancho máximo de rutas para alinear columnas
    let max_path_len = report
        .files
        .iter()
        .map(|f| display_path(&f.file).len())
        .max()
        .unwrap_or(20)
        .max(20);

    for fc in &report.files {
        print_file_row(fc, max_path_len);
    }

    println!("{separator}");
    print_total_row(report, max_path_len);
    println!();
    print_verdict(report.percentage(), min_coverage);
    println!();
}

fn print_file_row(fc: &FileCoverage, path_col_width: usize) {
    let pct = fc.percentage();
    let path = display_path(&fc.file);
    let bar = make_bar(pct);
    let color = pct_color(pct);

    println!(
        "  {DIM}{:<width$}{RESET}  {color}{}{RESET}  {BOLD}{color}{:>3.0}%{RESET}  {DIM}({}/{}){RESET}",
        path,
        bar,
        pct,
        fc.documented,
        fc.total_public,
        width = path_col_width,
    );
}

fn print_total_row(report: &CoverageReport, path_col_width: usize) {
    let pct = report.percentage();
    let bar = make_bar(pct);
    let color = pct_color(pct);
    let label = "TOTAL";

    println!(
        "  {BOLD}{:<width$}{RESET}  {color}{}{RESET}  {BOLD}{color}{:>3.0}%{RESET}  {DIM}({}/{}){RESET}",
        label,
        bar,
        pct,
        report.total_documented,
        report.total_public,
        width = path_col_width,
    );
}

fn print_verdict(pct: f64, min_coverage: u8) {
    if pct >= f64::from(min_coverage) {
        println!(
            "  {GREEN}{BOLD}✓ {:.0}% — Cobertura por encima del umbral mínimo ({min_coverage}%).{RESET}",
            pct
        );
    } else {
        println!(
            "  {RED}{BOLD}✗ {:.0}% — Por debajo del umbral mínimo ({min_coverage}%).{RESET}",
            pct
        );
        println!(
            "  {DIM}  Añade anotaciones /// @docs: [id] a las funciones públicas sin documentar.{RESET}"
        );
    }
}

// ── Utilidades ────────────────────────────────────────────────────────────────

fn make_bar(pct: f64) -> String {
    let filled = ((pct / 100.0) * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let empty = BAR_WIDTH - filled;
    format!(
        "{}{}",
        BAR_FILLED.to_string().repeat(filled),
        BAR_EMPTY.to_string().repeat(empty)
    )
}

fn pct_color(pct: f64) -> &'static str {
    if pct >= 80.0 {
        GREEN
    } else if pct >= 50.0 {
        YELLOW
    } else {
        RED
    }
}

/// Muestra la ruta relativa si es posible, o el nombre del archivo como fallback.
fn display_path(path: &Path) -> String {
    // Intentar ruta relativa al directorio actual
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(rel) = path.strip_prefix(&cwd) {
            return rel.display().to_string();
        }
    }
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_bar_full() {
        let bar = make_bar(100.0);
        assert_eq!(bar.chars().filter(|&c| c == BAR_FILLED).count(), BAR_WIDTH);
        assert_eq!(bar.chars().filter(|&c| c == BAR_EMPTY).count(), 0);
    }

    #[test]
    fn make_bar_empty() {
        let bar = make_bar(0.0);
        assert_eq!(bar.chars().filter(|&c| c == BAR_FILLED).count(), 0);
        assert_eq!(bar.chars().filter(|&c| c == BAR_EMPTY).count(), BAR_WIDTH);
    }

    #[test]
    fn make_bar_half() {
        let bar = make_bar(50.0);
        let filled = bar.chars().filter(|&c| c == BAR_FILLED).count();
        assert_eq!(filled, BAR_WIDTH / 2);
    }

    #[test]
    fn pct_color_thresholds() {
        assert_eq!(pct_color(100.0), GREEN);
        assert_eq!(pct_color(80.0), GREEN);
        assert_eq!(pct_color(79.9), YELLOW);
        assert_eq!(pct_color(50.0), YELLOW);
        assert_eq!(pct_color(49.9), RED);
        assert_eq!(pct_color(0.0), RED);
    }

    #[test]
    fn file_coverage_percentage() {
        let fc = FileCoverage {
            file: PathBuf::from("test.rs"),
            total_public: 10,
            documented: 8,
        };
        assert!((fc.percentage() - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn file_coverage_zero_public() {
        let fc = FileCoverage {
            file: PathBuf::from("test.rs"),
            total_public: 0,
            documented: 0,
        };
        assert_eq!(fc.percentage(), 100.0);
    }

    #[test]
    fn report_percentage_aggregates_correctly() {
        let report = CoverageReport {
            files: vec![],
            total_public: 20,
            total_documented: 15,
        };
        assert!((report.percentage() - 75.0).abs() < f64::EPSILON);
    }
}
