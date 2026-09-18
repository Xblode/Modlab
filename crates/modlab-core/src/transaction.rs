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
use serde::Serialize;

#[derive(Serialize)]
struct TransactionPayload<'a> {
    kind: &'a str,
    steps: &'a [String],
}

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
        let payload = serde_json::to_string(&TransactionPayload {
            kind: &self.kind,
            steps: &self.steps,
        })?;
        self.core.log_event("transaction.committed", &payload)
    }
}

pub const SNAPSHOT_EVENT: &str = "loadout.snapshot";
pub const ROLLBACK_EVENT: &str = "loadout.rollback";

#[derive(serde::Serialize)]
struct SnapshotPayload<'a> {
    loadout_id: i64,
    count: usize,
    entries: &'a [(i64, bool, i64)],
}

#[derive(serde::Deserialize)]
struct SnapshotPayloadOwned {
    loadout_id: i64,
    #[serde(default)]
    entries: Vec<(i64, bool, i64)>,
}

#[derive(serde::Serialize)]
struct RollbackPayload {
    loadout_id: i64,
    restored_from: i64,
    restored: usize,
}

/// Capture the loadout's current configuration — which mods, enabled or not,
/// in which order — as an immutable event. Per ADR 0003 this snapshots the
/// *state in base*, not file bytes: the CAS-backed content snapshot is a
/// separate, later step and this function must not pretend otherwise.
pub fn snapshot_loadout(core: &Core, loadout_id: i64) -> Result<i64> {
    let entries = core.select_loadout_state(loadout_id)?;
    let count = entries.len();
    let payload = serde_json::to_string(&SnapshotPayload {
        loadout_id,
        count,
        entries: &entries,
    })?;
    core.log_event(SNAPSHOT_EVENT, &payload)
}

/// Restore the loadout to its most recent snapshot. Returns the number of
/// entries restored. Refuses when no snapshot exists — falling back to
/// "empty loadout" would silently destroy the current state, the exact kind
/// of loss §5 exists to prevent.
pub fn rollback_loadout(core: &Core, loadout_id: i64) -> Result<usize> {
    let events = core.select_events(&[SNAPSHOT_EVENT], 500)?;
    let mut latest: Option<(i64, SnapshotPayloadOwned)> = None;
    for (id, _, _, payload) in events {
        let parsed = serde_json::from_str::<SnapshotPayloadOwned>(&payload)
            .map_err(|e| crate::CoreError::Message(format!("snapshot illisible : {e}")))?;
        if parsed.loadout_id == loadout_id {
            latest = Some((id, parsed));
            break;
        }
    }
    let (event_id, snap) = latest.ok_or_else(|| {
        crate::CoreError::Message("aucun instantané pour cette instance".to_string())
    })?;

    let entries: Vec<(i64, bool, i64)> = snap.entries;
    let restored = entries.len();
    core.replace_loadout_entries(loadout_id, &entries)?;

    let payload = serde_json::to_string(&RollbackPayload {
        loadout_id,
        restored_from: event_id,
        restored,
    })?;
    core.log_event(ROLLBACK_EVENT, &payload)?;
    Ok(restored)
}

/// One journaled mutation as the UI's transactions panel lists it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRecord {
    pub id: i64,
    pub occurred_at: String,
    /// The mutation's own vocabulary (`mod.import`, `loadout.snapshot`, …) —
    /// the UI maps ids to French labels and falls back to the raw id.
    pub kind: String,
    pub steps: Vec<String>,
}

const JOURNALED_KINDS: [&str; 3] = ["transaction.committed", SNAPSHOT_EVENT, ROLLBACK_EVENT];

/// The recent journal, newest first.
pub fn list(core: &Core, limit: usize) -> Result<Vec<TransactionRecord>> {
    let events = core.select_events(&JOURNALED_KINDS, limit)?;
    Ok(events
        .into_iter()
        .map(|(id, occurred_at, kind, payload)| {
            let (label, steps) = match kind.as_str() {
                "transaction.committed" => {
                    #[derive(serde::Deserialize)]
                    struct Committed {
                        kind: String,
                        #[serde(default)]
                        steps: Vec<String>,
                    }
                    match serde_json::from_str::<Committed>(&payload) {
                        Ok(c) => (c.kind, c.steps),
                        Err(_) => (kind.clone(), vec![payload]),
                    }
                }
                SNAPSHOT_EVENT => {
                    #[derive(serde::Deserialize)]
                    struct Snap {
                        count: usize,
                    }
                    match serde_json::from_str::<Snap>(&payload) {
                        Ok(s) => (
                            kind.clone(),
                            vec![format!("{} entrée(s) d'instance capturée(s)", s.count)],
                        ),
                        Err(_) => (kind.clone(), vec![payload]),
                    }
                }
                _ => {
                    #[derive(serde::Deserialize)]
                    struct Roll {
                        restored: usize,
                        restored_from: i64,
                    }
                    match serde_json::from_str::<Roll>(&payload) {
                        Ok(r) => (
                            kind.clone(),
                            vec![format!(
                                "{} entrée(s) restaurée(s) depuis l'instantané #{}",
                                r.restored, r.restored_from
                            )],
                        ),
                        Err(_) => (kind.clone(), vec![payload]),
                    }
                }
            };
            TransactionRecord {
                id,
                occurred_at,
                kind: label,
                steps,
            }
        })
        .collect())
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

    #[test]
    fn snapshot_then_rollback_restores_the_configuration() {
        let core = Core::open_in_memory().unwrap();
        let lo = crate::loadout::create(&core, "skyrimse", "Test").unwrap();

        let dir = std::env::temp_dir().join(format!("modlab-tx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), b"x").unwrap();

        let imported = crate::loadout::import_mods(
            &core,
            lo.id,
            &[
                crate::loadout::ImportEntry {
                    name: "A".into(),
                    source_path: dir.clone(),
                    enabled: true,
                },
                crate::loadout::ImportEntry {
                    name: "B".into(),
                    source_path: dir.clone(),
                    enabled: true,
                },
            ],
        )
        .unwrap();
        assert_eq!(imported, 2);

        snapshot_loadout(&core, lo.id).unwrap();

        // Mutate away from the snapshot: one mod disabled.
        let rows = crate::loadout::entries(&core, lo.id).unwrap();
        let a_id = rows.iter().find(|r| r.name == "A").unwrap().mod_id;
        crate::loadout::set_enabled(&core, lo.id, a_id, false).unwrap();
        assert!(
            !crate::loadout::entries(&core, lo.id)
                .unwrap()
                .iter()
                .find(|r| r.mod_id == a_id)
                .unwrap()
                .enabled
        );

        let restored = rollback_loadout(&core, lo.id).unwrap();
        assert_eq!(restored, 2);
        let rows = crate::loadout::entries(&core, lo.id).unwrap();
        assert!(
            rows.iter().find(|r| r.mod_id == a_id).unwrap().enabled,
            "rollback restores the enabled flag"
        );

        let journal = list(&core, 10).unwrap();
        assert!(journal.iter().any(|r| r.kind == ROLLBACK_EVENT));
        let first = journal.first().unwrap();
        assert_eq!(first.kind, ROLLBACK_EVENT);
        assert!(
            first.steps[0].contains("2 entrée(s)"),
            "the rollback card says what it restored: {:?}",
            first.steps
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rollback_without_a_snapshot_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = crate::loadout::create(&core, "skyrimse", "Test").unwrap();
        let err = rollback_loadout(&core, lo.id).unwrap_err();
        assert!(
            err.to_string().contains("aucun instantané"),
            "the refusal names its reason: {err}"
        );
    }
}
