//! Tipos de dominio principales de DocsGuard.
//!
//! Estas estructuras representan las entidades extraídas del código y la documentación,
//! así como los resultados de validación.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Representa un argumento extraído, ya sea del código fuente o de la documentación.
/// Estructura normalizada común para ambas fuentes (Blueprint §4.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arg {
    pub name: String,
    pub type_name: Option<String>,
    pub description: Option<String>,
}

/// Entidad de código extraída por tree-sitter.
/// Representa una función/método con su anotación `@docs` vinculada.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeEntity {
    /// Nombre de la función o método.
    pub name: String,
    /// Argumentos extraídos del AST.
    pub args: Vec<Arg>,
    /// Tipo de retorno, si existe.
    pub return_type: Option<String>,
    /// ID de documentación vinculado (extraído de `/// @docs: [id]`).
    pub doc_id: Option<String>,
    /// Ruta del archivo fuente.
    pub file_path: PathBuf,
    /// Línea donde se declaró la función.
    pub line: usize,
}

/// Sección de documentación extraída por pulldown-cmark.
/// Vinculada mediante un comentario HTML `<!-- @docs-id: xxx -->`.
#[derive(Debug, Clone, PartialEq)]
pub struct DocSection {
    /// Identificador único de la sección (extraído de `<!-- @docs-id: xxx -->`).
    pub id: String,
    /// Título de la sección (heading más cercano).
    pub title: Option<String>,
    /// Argumentos documentados en la sección.
    pub args: Vec<Arg>,
    /// Ruta del archivo de documentación.
    pub file_path: PathBuf,
    /// Línea donde se encontró el marcador de ID.
    pub line: usize,
}

/// Severidad de un hallazgo de validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "Error"),
            Severity::Warning => write!(f, "Warning"),
            Severity::Info => write!(f, "Info"),
        }
    }
}

/// Resultado de validación individual.
/// Sigue el principio "El Error es el Producto" (Blueprint §7):
/// cada resultado incluye contexto accionable.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub severity: Severity,
    /// Mensaje principal del hallazgo.
    pub message: String,
    /// Nombre de la función afectada.
    pub function_name: Option<String>,
    /// Ubicación en el código fuente.
    pub code_location: Option<String>,
    /// ID de documentación vinculado.
    pub doc_id: Option<String>,
    /// Consejo accionable para el desarrollador.
    pub hint: Option<String>,
}

impl std::fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let icon = match self.severity {
            Severity::Error => "[X]",
            Severity::Warning => "[!]",
            Severity::Info => "[i]",
        };

        write!(f, "{} {}", icon, self.severity)?;

        if let Some(ref func) = self.function_name {
            if let Some(ref loc) = self.code_location {
                write!(f, " en fn {} ({})", func, loc)?;
            } else {
                write!(f, " en fn {}", func)?;
            }
        }

        writeln!(f)?;
        writeln!(f, "    -> {}", self.message)?;

        if let Some(ref doc_id) = self.doc_id {
            writeln!(f, "    -> ID vinculado: '{}'", doc_id)?;
        }

        if let Some(ref hint) = self.hint {
            writeln!(f, "    -> Sugerencia: {}", hint)?;
        }

        Ok(())
    }
}
