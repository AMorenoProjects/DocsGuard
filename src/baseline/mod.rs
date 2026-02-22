//! Sistema de Baseline para "Green Build Day 1" (Blueprint §1.3, §5 Semana 3).
//!
//! Permite volcar todos los errores existentes a `.docsguard/baseline.yaml`.
//! El comando `check` resta los errores del baseline del resultado,
//! bloqueando solo regresiones nuevas.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::core::types::{Severity, ValidationResult};

/// Nombre del directorio de configuración.
const DOCSGUARD_DIR: &str = ".docsguard";
/// Nombre del archivo de baseline.
const BASELINE_FILE: &str = "baseline.yaml";

/// Entrada individual en el baseline.
/// Identifica un error conocido que debe ignorarse.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BaselineEntry {
    /// Severidad del hallazgo original.
    pub severity: String,
    /// Nombre de la función (si aplica).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,
    /// ID de documentación vinculado.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    /// Fingerprint del mensaje (primeras palabras para estabilidad).
    pub message_fingerprint: String,
}

/// Contenido del archivo baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// Versión del formato.
    pub version: String,
    /// Fecha de generación.
    pub generated_at: String,
    /// Entradas del baseline.
    pub entries: Vec<BaselineEntry>,
}

impl Baseline {
    /// Crea un baseline nuevo desde una lista de resultados de validación.
    pub fn from_results(results: &[ValidationResult]) -> Self {
        let entries: Vec<BaselineEntry> = results
            .iter()
            .filter(|r| r.severity != Severity::Info)
            .map(|r| BaselineEntry {
                severity: r.severity.to_string(),
                function_name: r.function_name.clone(),
                doc_id: r.doc_id.clone(),
                message_fingerprint: make_fingerprint(&r.message),
            })
            .collect();

        Baseline {
            version: "1".into(),
            generated_at: chrono_now(),
            entries,
        }
    }

    /// Carga un baseline desde el directorio del proyecto.
    pub fn load(project_root: &Path) -> Result<Option<Self>> {
        let path = baseline_path(project_root);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("No se pudo leer el baseline: {}", path.display()))?;

        let baseline: Baseline = serde_yml::from_str(&content)
            .with_context(|| format!("Error al parsear el baseline: {}", path.display()))?;

        if baseline.version != "1" {
            anyhow::bail!(
                "Versión de baseline no soportada: '{}' (esperada: '1')\n    -> Archivo: {}",
                baseline.version,
                path.display()
            );
        }

        Ok(Some(baseline))
    }

    /// Guarda el baseline al disco.
    pub fn save(&self, project_root: &Path) -> Result<PathBuf> {
        let dir = project_root.join(DOCSGUARD_DIR);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("No se pudo crear: {}", dir.display()))?;
        }

        let path = dir.join(BASELINE_FILE);
        let content = serde_yml::to_string(self).context("Error al serializar el baseline")?;

        std::fs::write(&path, content)
            .with_context(|| format!("No se pudo escribir: {}", path.display()))?;

        Ok(path)
    }

    /// Convierte las entradas a un HashSet para comparación rápida.
    fn entry_set(&self) -> HashSet<BaselineEntry> {
        self.entries.iter().cloned().collect()
    }
}

/// Filtra los resultados de validación, eliminando los que están en el baseline.
/// Retorna: (resultados_nuevos, total_filtrados)
pub fn filter_baseline(
    results: &[ValidationResult],
    baseline: &Baseline,
) -> (Vec<ValidationResult>, usize) {
    let known = baseline.entry_set();
    let mut filtered = 0;

    let new_results: Vec<ValidationResult> = results
        .iter()
        .filter(|r| {
            if r.severity == Severity::Info {
                return true; // Info siempre pasa
            }

            let entry = BaselineEntry {
                severity: r.severity.to_string(),
                function_name: r.function_name.clone(),
                doc_id: r.doc_id.clone(),
                message_fingerprint: make_fingerprint(&r.message),
            };

            if known.contains(&entry) {
                filtered += 1;
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();

    (new_results, filtered)
}

/// Genera una huella del mensaje para comparación estable.
/// Usa las primeras 6 palabras significativas para evitar falsos negativos
/// por cambios menores en los mensajes.
fn make_fingerprint(message: &str) -> String {
    message
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Obtiene la ruta al archivo baseline.
fn baseline_path(project_root: &Path) -> PathBuf {
    project_root.join(DOCSGUARD_DIR).join(BASELINE_FILE)
}

/// Timestamp simple sin dependencia de chrono.
fn chrono_now() -> String {
    // Usar timestamp del sistema
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}", duration.as_secs())
}

/// Ejecuta el comando baseline: vuelca errores actuales al archivo.
pub fn run_baseline(code_file: &Path, doc_file: &Path, project_root: &Path) -> Result<()> {
    if !code_file.exists() {
        anyhow::bail!("Archivo de código no encontrado: {}", code_file.display());
    }
    if !doc_file.exists() {
        anyhow::bail!(
            "Archivo de documentación no encontrado: {}",
            doc_file.display()
        );
    }

    println!("DocsGuard Baseline — Volcando errores existentes\n");

    let code_entities = crate::parser::code_parser::parse_code_file(code_file)
        .context("Error al parsear el archivo de código")?;
    let doc_sections = crate::parser::doc_parser::parse_markdown_file(doc_file)
        .context("Error al parsear el archivo de documentación")?;

    let results = crate::core::validator::validate_links(&code_entities, &doc_sections);
    let baseline = Baseline::from_results(&results);

    let entry_count = baseline.entries.len();
    let path = baseline.save(project_root)?;

    println!(
        "  {} errores/advertencias volcados al baseline.",
        entry_count
    );
    println!("  Archivo: {}", path.display());
    println!("\n  El CI ahora pasará en verde. Solo se bloquearán regresiones nuevas.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ValidationResult;

    fn make_result(
        severity: Severity,
        msg: &str,
        func: Option<&str>,
        doc_id: Option<&str>,
    ) -> ValidationResult {
        ValidationResult {
            severity,
            message: msg.into(),
            function_name: func.map(String::from),
            code_location: None,
            doc_id: doc_id.map(String::from),
            hint: None,
        }
    }

    #[test]
    fn baseline_filters_known_errors() {
        let results = vec![
            make_result(
                Severity::Error,
                "ID de documentación 'auth-login' no encontrado en el archivo de docs.",
                Some("login"),
                Some("auth-login"),
            ),
            make_result(
                Severity::Warning,
                "Sección de documentación 'Logout' no está vinculada desde ninguna función.",
                None,
                Some("auth-logout"),
            ),
            make_result(
                Severity::Error,
                "Este es un error nuevo que no estaba antes.",
                Some("new_fn"),
                Some("new-id"),
            ),
        ];

        // Crear baseline con los primeros 2
        let baseline = Baseline::from_results(&results[..2]);
        assert_eq!(baseline.entries.len(), 2);

        // Filtrar: solo el tercer error debería sobrevivir
        let (new_results, filtered) = filter_baseline(&results, &baseline);
        assert_eq!(filtered, 2);
        let new_errors: Vec<_> = new_results
            .iter()
            .filter(|r| r.severity == Severity::Error)
            .collect();
        assert_eq!(new_errors.len(), 1);
        assert!(new_errors[0].message.contains("error nuevo"));
    }

    #[test]
    fn empty_baseline_passes_everything() {
        let results = vec![make_result(Severity::Error, "Un error", Some("fn_a"), None)];

        let baseline = Baseline {
            version: "1".into(),
            generated_at: "test".into(),
            entries: vec![],
        };

        let (new_results, filtered) = filter_baseline(&results, &baseline);
        assert_eq!(filtered, 0);
        assert_eq!(new_results.len(), 1);
    }

    #[test]
    fn baseline_round_trip() {
        let results = vec![make_result(
            Severity::Error,
            "Test error message here",
            Some("test_fn"),
            Some("test-id"),
        )];

        let baseline = Baseline::from_results(&results);
        let yaml = serde_yml::to_string(&baseline).unwrap();
        let loaded: Baseline = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].function_name.as_deref(), Some("test_fn"));
    }
}
