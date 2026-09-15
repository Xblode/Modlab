//! Transactions — the safety mechanism behind every mutation.
//! See docs/01-architecture-decidee.md §5: `plan → approve → snapshot →
//! apply → verify → commit | rollback`.
//!
//! This is a first, deliberately thin slice: it journals what happened as
//! a `Transaction`, one step at a time, and commits it as an event. The
//! full cycle — a separate approval step, snapshotting to a revision
//! before applying, verification, and automatic rollback on failure —
//! needs the store/revision model to exist first and is real design work
//! for `core-dev`, not something to bolt on here. Treat this as the seam
//! everything else will plug into, not the finished mechanism.

use crate::{Core, Result};

/// A transaction under construction. Add steps describing what will
/// happen, then `commit()` to journal it. Nothing is applied by this type
/// yet — callers perform the actual filesystem/database work and record
/// what they did as steps, so the journal stays an honest record.
pub struct Transaction<'a> {
    core: &'a Core,
    kind: String,
    steps: Vec<String>,
}

impl<'a> Transaction<'a> {
    pub fn new(core: &'a Core, kind: impl Into<String>) -> Self {
        Self {
            core,
            kind: kind.into(),
            steps: Vec::new(),
        }
    }

    /// Record one step that happened as part of this transaction.
    pub fn step(&mut self, description: impl Into<String>) -> &mut Self {
        self.steps.push(description.into());
        self
    }

    /// Journal the transaction as committed. Returns the event id.
    pub fn commit(self) -> Result<i64> {
        let payload = format!(
            "{{\"kind\":{:?},\"steps\":{:?}}}",
            self.kind, self.steps
        );
        self.core.log_event("transaction.committed", &payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_steps_and_commits_as_one_event() {
        let core = Core::open_in_memory().unwrap();
        let before = core.event_count().unwrap();

        let mut tx = Transaction::new(&core, "mod.import");
        tx.step("hashed archive.7z")
            .step("stored 42 files in the CAS")
            .step("created mod 'Example Mod'");
        tx.commit().unwrap();

        assert_eq!(core.event_count().unwrap(), before + 1);
    }
}
