//! SigmaOS iloveapi — file conversion API abstraction.
//!
//! Provider-agnostic conversion job manager: jobs track source file,
//! target format, provider, and status. Actual conversion is delegated to
//! a `ConversionProvider` implementation (HTTP, local tool, mock).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Errors produced by this component.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("job not found: {0}")]
    NotFound(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("unsupported conversion: {0} -> {1}")]
    Unsupported(String, String),
}

/// Conversion job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Processing,
    Done,
    Failed,
}

/// A conversion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertJob {
    pub id: String,
    pub source: PathBuf,
    pub target_format: String,
    pub provider: String,
    #[serde(default = "default_status")]
    pub status: JobStatus,
    #[serde(default)]
    pub output: Option<PathBuf>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub created_unix: u64,
}

fn default_status() -> JobStatus {
    JobStatus::Queued
}

/// Provider trait — implement for HTTP APIs or local tools.
pub trait ConversionProvider {
    /// Provider name.
    fn name(&self) -> &str;
    /// Check if provider supports source_ext -> target_format.
    fn supports(&self, source_ext: &str, target_format: &str) -> bool;
    /// Execute conversion; returns output path on success.
    fn convert(
        &self,
        source: &Path,
        target_format: &str,
        output_dir: &Path,
    ) -> Result<PathBuf, ConvertError>;
}

/// Job manager.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobManager {
    #[serde(default)]
    pub jobs: BTreeMap<String, ConvertJob>,
}

impl JobManager {
    /// Create a new job (validates source exists and extension non-empty).
    pub fn create(
        &mut self,
        source: PathBuf,
        target_format: &str,
        provider: &str,
    ) -> Result<ConvertJob, ConvertError> {
        if !source.is_file() {
            return Err(ConvertError::Validation(format!(
                "source not a file: {}",
                source.display()
            )));
        }
        if target_format.is_empty() {
            return Err(ConvertError::Validation("empty target format".into()));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let job = ConvertJob {
            id: id.clone(),
            source,
            target_format: target_format.to_lowercase(),
            provider: provider.to_string(),
            status: JobStatus::Queued,
            output: None,
            error: None,
            created_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        self.jobs.insert(id.clone(), job.clone());
        Ok(job)
    }

    /// Get job by id.
    pub fn get(&self, id: &str) -> Option<&ConvertJob> {
        self.jobs.get(id)
    }

    /// List jobs, optionally filtered by status.
    pub fn list(&self, status: Option<JobStatus>) -> Vec<&ConvertJob> {
        self.jobs
            .values()
            .filter(|j| status.map(|s| j.status == s).unwrap_or(true))
            .collect()
    }

    /// Execute a job with the given provider; updates status/output/error.
    pub fn run<P: ConversionProvider>(
        &mut self,
        id: &str,
        provider: &P,
        output_dir: &Path,
    ) -> Result<(), ConvertError> {
        let job = self
            .jobs
            .get_mut(id)
            .ok_or_else(|| ConvertError::NotFound(id.into()))?;
        let source_ext = job
            .source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !provider.supports(&source_ext, &job.target_format) {
            let msg = format!("unsupported: {source_ext} -> {}", job.target_format);
            job.status = JobStatus::Failed;
            job.error = Some(msg.clone());
            return Err(ConvertError::Unsupported(
                source_ext,
                job.target_format.clone(),
            ));
        }
        job.status = JobStatus::Processing;
        let source = job.source.clone();
        let target = job.target_format.clone();
        match provider.convert(&source, &target, output_dir) {
            Ok(out) => {
                job.status = JobStatus::Done;
                job.output = Some(out);
                job.error = None;
                Ok(())
            }
            Err(e) => {
                job.status = JobStatus::Failed;
                job.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Remove a job.
    pub fn remove(&mut self, id: &str) -> Result<(), ConvertError> {
        if self.jobs.remove(id).is_none() {
            return Err(ConvertError::NotFound(id.into()));
        }
        Ok(())
    }

    /// Save to JSON.
    pub fn save(&self, path: &Path) -> Result<(), ConvertError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Load from JSON.
    pub fn load(path: &Path) -> Result<Self, ConvertError> {
        let text = std::fs::read_to_string(path)
            .map_err(|_| ConvertError::NotFound(path.display().to_string()))?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Default job store path.
pub fn default_store_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "sigmaos", "sigmaos")
        .map(|p| p.data_dir().join("iloveapi").join("jobs.json"))
}

/// Default output directory for conversions.
pub fn default_output_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "sigmaos", "sigmaos")
        .map(|p| p.data_dir().join("iloveapi").join("output"))
}

/// A mock provider for testing / offline use: "converts" by copying the
/// file with a new extension.
pub struct CopyProvider;

impl ConversionProvider for CopyProvider {
    fn name(&self) -> &str {
        "copy"
    }
    fn supports(&self, _src: &str, _dst: &str) -> bool {
        true
    }
    fn convert(
        &self,
        source: &Path,
        target_format: &str,
        output_dir: &Path,
    ) -> Result<PathBuf, ConvertError> {
        std::fs::create_dir_all(output_dir)?;
        let stem = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let out = output_dir.join(format!("{stem}.{target_format}"));
        std::fs::copy(source, &out)?;
        Ok(out)
    }
}
