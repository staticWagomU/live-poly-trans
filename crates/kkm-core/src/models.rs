//! Finding the model files.
//!
//! Three models are loaded at runtime — the recogniser, the VAD, and the
//! translator — and each was resolved its own way while the pipeline grew
//! (an env var here, a build-time path there). This is the one place that
//! decides, so the eventual download manager has a single thing to feed:
//! it will add a directory to the search list, not another special case.

use std::path::{Path, PathBuf};

/// The default file name for a model the app ships with a choice of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// The speech recogniser (whisper GGML).
    Asr,
    /// Silero VAD, in whisper.cpp's GGML packaging.
    Vad,
    /// The translation LLM (GGUF).
    Translator,
}

impl Model {
    /// The environment variable that overrides this model outright, with a
    /// full path. Kept from the pipeline's env-var era: it is how a
    /// measurement run swaps in a different model size.
    pub fn env_var(self) -> &'static str {
        match self {
            Model::Asr => "KKM_WHISPER_MODEL",
            Model::Vad => "KKM_VAD_MODEL",
            Model::Translator => "KKM_LLM_MODEL",
        }
    }

    /// The file looked for in each search directory.
    pub fn default_file(self) -> &'static str {
        match self {
            Model::Asr => "ggml-large-v3-turbo-q8_0.bin",
            Model::Vad => "ggml-silero-v5.1.2.bin",
            Model::Translator => "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        }
    }
}

/// Resolves a model to a path that exists, searching the given directories in
/// order. The first directory is the one a download would land in; the last is
/// the checkout's `models/`, which is what makes `cargo run` work.
pub struct ModelManager {
    dirs: Vec<PathBuf>,
}

impl ModelManager {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    /// Where `model` is, or an error naming every place that was looked at —
    /// "model not found" without the list is a support ticket.
    pub fn resolve(&self, model: Model) -> anyhow::Result<PathBuf> {
        self.resolve_with(model, |name| std::env::var(name).ok(), |p| p.exists())
    }

    /// The decision itself, with the environment and the filesystem injected
    /// so it can be tested without either.
    fn resolve_with(
        &self,
        model: Model,
        env: impl Fn(&str) -> Option<String>,
        exists: impl Fn(&Path) -> bool,
    ) -> anyhow::Result<PathBuf> {
        // An explicit override is used as given: pointing it at a missing
        // file is a mistake worth reporting, not a reason to quietly load a
        // different model than the one that was asked for.
        if let Some(path) = env(model.env_var()) {
            let path = PathBuf::from(path);
            anyhow::ensure!(
                exists(&path),
                "{} points at {}, which does not exist",
                model.env_var(),
                path.display()
            );
            return Ok(path);
        }
        let file = model.default_file();
        for dir in &self.dirs {
            let candidate = dir.join(file);
            if exists(&candidate) {
                return Ok(candidate);
            }
        }
        anyhow::bail!(
            "no {file} in {} (set {} to use another path)",
            self.dirs
                .iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            model.env_var()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> ModelManager {
        ModelManager::new(vec![PathBuf::from("/data"), PathBuf::from("/repo/models")])
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn the_first_directory_that_has_the_file_wins() {
        let found = manager()
            .resolve_with(Model::Asr, no_env, |p| p.starts_with("/repo"))
            .unwrap();
        assert_eq!(
            found,
            PathBuf::from("/repo/models/ggml-large-v3-turbo-q8_0.bin")
        );
    }

    #[test]
    fn a_downloaded_model_shadows_the_checkouts_copy() {
        let found = manager()
            .resolve_with(Model::Vad, no_env, |_| true)
            .unwrap();
        assert_eq!(found, PathBuf::from("/data/ggml-silero-v5.1.2.bin"));
    }

    #[test]
    fn an_env_override_wins_over_every_directory() {
        let found = manager()
            .resolve_with(
                Model::Translator,
                |name| (name == "KKM_LLM_MODEL").then(|| "/tmp/other.gguf".to_string()),
                |_| true,
            )
            .unwrap();
        assert_eq!(found, PathBuf::from("/tmp/other.gguf"));
    }

    #[test]
    fn an_override_pointing_nowhere_is_an_error_not_a_fallback() {
        // Loading a different model than the one that was named would make a
        // measurement run silently meaningless.
        let err = manager()
            .resolve_with(
                Model::Asr,
                |_| Some("/tmp/missing.bin".to_string()),
                |p| !p.starts_with("/tmp"),
            )
            .unwrap_err()
            .to_string();
        assert!(err.contains("KKM_WHISPER_MODEL"), "{err}");
        assert!(err.contains("/tmp/missing.bin"), "{err}");
    }

    #[test]
    fn a_missing_model_names_everywhere_it_was_looked_for() {
        let err = manager()
            .resolve_with(Model::Asr, no_env, |_| false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("/data"), "{err}");
        assert!(err.contains("/repo/models"), "{err}");
        assert!(err.contains("ggml-large-v3-turbo-q8_0.bin"), "{err}");
    }
}
