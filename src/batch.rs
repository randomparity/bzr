use crate::error::{BzrError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BatchOutcome {
    pub succeeded: usize,
    pub failed: usize,
}

impl BatchOutcome {
    pub(crate) const fn new(succeeded: usize, failed: usize) -> Self {
        Self { succeeded, failed }
    }

    pub(crate) fn ensure_complete(self) -> Result<()> {
        if self.failed > 0 {
            Err(BzrError::BatchPartialFailure {
                succeeded: self.succeeded,
                failed: self.failed,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
