use std::time::Duration;

use crate::commands::runtime::inline_server::InlineServer;
use crate::types::{ApiMode, OutputFormat};

/// Per-invocation command settings that must cross command/module boundaries.
#[derive(Debug, Clone)]
pub struct CommandContext {
    server: Option<String>,
    format: OutputFormat,
    api: Option<ApiMode>,
    dry_run: bool,
    assume_yes: bool,
    inline_server: Option<InlineServer>,
    request_timeout: Duration,
    retry_max: u32,
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
            request_timeout: crate::http::REQUEST_TIMEOUT,
            retry_max: 0,
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
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    #[must_use]
    pub fn retry_max(&self) -> u32 {
        self.retry_max
    }
}
