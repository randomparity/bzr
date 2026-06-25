/// Per-command metadata used before command execution starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandCapabilities {
    supports_dry_run: bool,
    credential_requirement: Option<&'static str>,
}

impl CommandCapabilities {
    #[must_use]
    pub(crate) const fn anonymous() -> Self {
        Self {
            supports_dry_run: false,
            credential_requirement: None,
        }
    }

    #[must_use]
    pub(crate) const fn authenticated(command_name: &'static str) -> Self {
        Self {
            supports_dry_run: false,
            credential_requirement: Some(command_name),
        }
    }

    #[must_use]
    pub(crate) const fn dry_run_mutation(command_name: &'static str) -> Self {
        Self {
            supports_dry_run: true,
            credential_requirement: Some(command_name),
        }
    }

    #[must_use]
    pub(crate) const fn supports_dry_run(self) -> bool {
        self.supports_dry_run
    }

    #[must_use]
    pub(crate) const fn credential_requirement(self) -> Option<&'static str> {
        self.credential_requirement
    }
}
