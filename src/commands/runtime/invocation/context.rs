use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::commands::runtime::invocation::InlineServer;
use crate::types::output::{OutputFormat, ProgressFormat};
use crate::types::transport::ApiMode;

/// Per-invocation command settings that must cross command/module boundaries.
#[derive(Debug, Clone)]
pub struct CommandContext {
    server: Option<String>,
    format: OutputFormat,
    api: Option<ApiMode>,
    dry_run: bool,
    assume_yes: bool,
    inline_server: Option<InlineServer>,
    config_path_override: Option<PathBuf>,
    request_timeout: Duration,
    retry_max: u32,
    credential_requirement: Option<&'static str>,
    progress: Option<ProgressFormat>,
}

impl CommandContext {
    /// Build a context with default invocation flags and network tuning.
    #[must_use]
    pub fn new(server: Option<&str>, format: OutputFormat, api: Option<ApiMode>) -> Self {
        Self {
            server: server.map(str::to_owned),
            format,
            api,
            dry_run: false,
            assume_yes: false,
            inline_server: None,
            config_path_override: None,
            request_timeout: crate::http::REQUEST_TIMEOUT,
            retry_max: 0,
            credential_requirement: None,
            progress: None,
        }
    }

    #[must_use]
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    #[must_use]
    pub fn with_assume_yes(mut self, assume_yes: bool) -> Self {
        self.assume_yes = assume_yes;
        self
    }

    #[must_use]
    pub fn with_inline_server(mut self, inline_server: Option<InlineServer>) -> Self {
        self.inline_server = inline_server;
        self
    }

    #[must_use]
    pub fn with_config_path_override(mut self, config_path_override: Option<PathBuf>) -> Self {
        self.config_path_override = config_path_override;
        self
    }

    #[must_use]
    pub fn with_request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    #[must_use]
    pub fn with_retry_max(mut self, retry_max: u32) -> Self {
        self.retry_max = retry_max;
        self
    }

    #[must_use]
    pub fn with_progress(mut self, progress: Option<ProgressFormat>) -> Self {
        self.progress = progress;
        self
    }

    #[must_use]
    pub fn with_credential_requirement(
        mut self,
        credential_requirement: Option<&'static str>,
    ) -> Self {
        self.credential_requirement = credential_requirement;
        self
    }

    #[must_use]
    pub fn with_server(&self, server: Option<&str>) -> Self {
        let mut ctx = self.clone();
        ctx.server = server.map(str::to_owned);
        ctx
    }

    #[must_use]
    pub fn server(&self) -> Option<&str> {
        self.server.as_deref()
    }

    #[must_use]
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    #[must_use]
    pub fn api(&self) -> Option<ApiMode> {
        self.api
    }

    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    #[must_use]
    pub fn assume_yes(&self) -> bool {
        self.assume_yes
    }

    #[must_use]
    pub fn inline_server(&self) -> Option<&InlineServer> {
        self.inline_server.as_ref()
    }

    #[must_use]
    pub fn config_path_override(&self) -> Option<&Path> {
        self.config_path_override.as_deref()
    }

    #[must_use]
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    #[must_use]
    pub fn retry_max(&self) -> u32 {
        self.retry_max
    }

    #[must_use]
    pub fn credential_requirement(&self) -> Option<&'static str> {
        self.credential_requirement
    }

    #[must_use]
    pub fn progress(&self) -> Option<ProgressFormat> {
        self.progress
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
