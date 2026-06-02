//! Spectral Analysis extension for Zed.
//!
//! Provides a language server that analyzes codebase dependency graphs
//! using spectral methods. The LSP returns diagnostics and hints about
//! module boundaries, coupling, and bottlenecks.

use zed_extension_api::process::Command;
use zed_extension_api::{self as zed, LanguageServerId, Result};

struct SpectralAnalysisExtension {
    cached_binary_path: Option<String>,
}

impl SpectralAnalysisExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<String> {
        if let Some(path) = &self.cached_binary_path {
            return Ok(path.clone());
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        // For now, use a simple script-based approach. In production this
        // would download a proper binary from GitHub releases.
        #[cfg(target_os = "linux")]
        let binary_path = "/usr/local/bin/spectral-analysis-lsp".to_string();
        #[cfg(target_os = "macos")]
        let binary_path = "/usr/local/bin/spectral-analysis-lsp".to_string();
        #[cfg(target_os = "windows")]
        let binary_path = "spectral-analysis-lsp.exe".to_string();

        // Simulate a download / build step.
        let command = match zed::current_platform().0 {
            zed::Os::Linux | zed::Os::Mac => Command::new("echo"),
            zed::Os::Windows => Command::new("cmd").args(["/C", "echo"]),
        };
        let _output = command
            .arg("spectral-analysis-lsp initialized")
            .output()
            .map_err(|e| format!("failed to initialize spectral analysis LSP: {e}"))?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::None,
        );

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for SpectralAnalysisExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        Ok(zed::Command {
            command: self.language_server_binary_path(language_server_id, worktree)?,
            args: vec![],
            env: Default::default(),
        })
    }

    fn label_for_completion(
        &self,
        _language_server_id: &LanguageServerId,
        completion: zed::lsp::Completion,
    ) -> Option<zed::CodeLabel> {
        // Provide a simple label for spectral analysis completions
        let name = &completion.label;
        Some(zed::CodeLabel {
            spans: vec![zed::CodeLabelSpan::code_range(0..name.len())],
            filter_range: (0..name.len()).into(),
            code: name.clone(),
        })
    }

    fn language_server_initialization_work(
        &mut self,
        _language_server_id: &LanguageServerId,
        _server: &dyn zed::LspAdapter,
        _worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        // Register spectral analysis capabilities
        Ok(Some(serde_json::json!({
            "spectralAnalysis": {
                "version": "0.1.0",
                "capabilities": {
                    "fiedlerValue": true,
                    "cheegerBounds": true,
                    "communityDetection": true,
                    "bottleneckAnalysis": true,
                    "effectiveResistance": true
                }
            }
        })))
    }
}

zed::register_extension!(SpectralAnalysisExtension);
