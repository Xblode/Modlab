//! Transactions — the safety mechanism behind every mutation.
//! See docs/01-architecture-decidee.md §5: `intention → PLAN (lisible) →
//! APPROBATION → SNAPSHOT → APPLY → VÉRIFICATION → COMMIT | ROLLBACK auto`.
//!
//! `Transaction` (below) is the raw primitive — journal these steps as one
//! committed event — kept exactly as it was for the handful of mutations
//! that don't touch a loadout's configuration (`loadout::create`,
//! `loadout::import_mods`: there's nothing meaningful to snapshot *before*
//! a loadout exists or before its first mods land). `Plan`/`ApprovedPlan` is
//! the actual full cycle: a plan is read-only and inert until a caller
//! explicitly `.approve()`s it — a type-level fence, not just convention —
//! and only an `ApprovedPlan` can `.apply()`, which captures the loadout's
//! state, runs the mutation, then a caller-supplied verification, and
//! restores that captured state automatically if either one fails.
//!
//! `snapshot_loadout`/`rollback_loadout` are a **separate, public,
//! human-facing** pair — a named revision a person chooses to create
//! (`ActiveInstanceCard`'s "Enregistrer") and can choose to return to
//! ("Annuler les modifications"), backing `revision_count`/`dirty_summary`
//! below. `ApprovedPlan::apply`'s internal safety capture deliberately does
//! **not** reuse this pair or its event kinds — seeing that decoupling in
//! `apply`'s own docs before changing it, because it wasn't the original
//! design: every `apply()` call briefly journaled a real `loadout.snapshot`
//! event, which meant every single mod toggle flooded the visible Journal
//! and inflated the revision count. Two different things needed two
//! different mechanisms once a second thing (visible revisions) needed one.
//!
//! Two boundaries this does *not* cross, written down so a future session
//! doesn't assume otherwise:
//! - **Approval is a type-level fence, not a policy.** Core doesn't decide
//!   *who* approves what or show a plan to a human — that's a client/MCP
//!   concern (§7.2's three permission tiers). Every current caller approves
//!   immediately after building the plan; the fence exists so that stops
//!   being true the moment a client needs it to, without touching this
//!   module.
//! - **The snapshot is state-in-base only** (which mods, enabled, in what
//!   order) — never file bytes, per ADR 0003. Verify and rollback restore a
//!   *configuration*, not disk content. Correct while every deployment
//!   backend either writes nothing to real disk (USVFS, ADR 0001) or
//!   doesn't exist yet; stops being correct the day a backend actually
//!   writes into a game folder — CAS ingestion (ADR 0003's "chemin de
//!   sortie") must land before that backend does.

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

#[derive(serde::Serialize, serde::Deserialize)]
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

/// How many snapshot events this scans through looking for one loadout's —
/// shared by every function below so `rollback_loadout`, `revision_count`
/// and `dirty_summary` can never disagree about how far back "recent"
/// reaches.
const SNAPSHOT_SCAN_LIMIT: usize = 5000;

/// The most recent snapshot recorded for `loadout_id`, if any —
/// `(event_id, entries)`. The one place that walks the event log looking for
/// a loadout's snapshots; `rollback_loadout`, `revision_count` and
/// `dirty_summary` all go through this so they can never disagree about
/// which snapshot is "latest".
fn latest_snapshot(core: &Core, loadout_id: i64) -> Result<Option<(i64, Vec<(i64, bool, i64)>)>> {
    let events = core.select_events(&[SNAPSHOT_EVENT], SNAPSHOT_SCAN_LIMIT)?;
    for (id, _, _, payload) in events {
        let parsed = serde_json::from_str::<SnapshotPayloadOwned>(&payload)
            .map_err(|e| crate::CoreError::Message(format!("snapshot illisible : {e}")))?;
        if parsed.loadout_id == loadout_id {
            return Ok(Some((id, parsed.entries)));
        }
    }
    Ok(None)
}

/// Restore the loadout to its most recent snapshot. Returns the number of
/// entries restored. Refuses when no snapshot exists — falling back to
/// "empty loadout" would silently destroy the current state, the exact kind
/// of loss §5 exists to prevent.
pub fn rollback_loadout(core: &Core, loadout_id: i64) -> Result<usize> {
    let (event_id, entries) = latest_snapshot(core, loadout_id)?.ok_or_else(|| {
        crate::CoreError::Message("aucun instantané pour cette instance".to_string())
    })?;

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

/// How many snapshots exist for this loadout — the revision number a
/// version string like `r.3` reports. Derived from the event log, never a
/// separately tracked counter that could drift from what `rollback_loadout`
/// actually restores: every snapshot counts, whether or not it is still the
/// *latest* one.
pub fn revision_count(core: &Core, loadout_id: i64) -> Result<i64> {
    let events = core.select_events(&[SNAPSHOT_EVENT], SNAPSHOT_SCAN_LIMIT)?;
    let count = events
        .into_iter()
        .filter_map(|(_, _, _, payload)| {
            serde_json::from_str::<SnapshotPayloadOwned>(&payload).ok()
        })
        .filter(|snap| snap.loadout_id == loadout_id)
        .count();
    Ok(i64::try_from(count).unwrap_or(i64::MAX))
}

/// How many mods changed state since the last snapshot — what a client
/// shows as "3 activés, 1 désactivé depuis r.5" before offering to save a
/// new revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirtySummary {
    /// Mods enabled now that weren't enabled in the latest snapshot
    /// (including a mod imported since that snapshot and already on).
    pub plus: i64,
    /// Mods disabled now that were enabled in the latest snapshot.
    pub minus: i64,
}

impl DirtySummary {
    #[must_use]
    pub fn is_clean(self) -> bool {
        self.plus == 0 && self.minus == 0
    }
}

/// Compare the loadout's live state to its latest snapshot. No snapshot yet
/// reads as clean — there is nothing to be dirty *against*, and reporting a
/// count against a snapshot that doesn't exist would be a made-up number.
pub fn dirty_summary(core: &Core, loadout_id: i64) -> Result<DirtySummary> {
    let Some((_, baseline)) = latest_snapshot(core, loadout_id)? else {
        return Ok(DirtySummary { plus: 0, minus: 0 });
    };
    let baseline_enabled: std::collections::HashMap<i64, bool> = baseline
        .into_iter()
        .map(|(mod_id, enabled, _)| (mod_id, enabled))
        .collect();

    let mut plus = 0i64;
    let mut minus = 0i64;
    for (mod_id, enabled, _) in core.select_loadout_state(loadout_id)? {
        match baseline_enabled.get(&mod_id) {
            Some(&was_enabled) if was_enabled != enabled => {
                if enabled {
                    plus += 1;
                } else {
                    minus += 1;
                }
            }
            // A mod imported after the snapshot: counts as "+1" only if it's
            // on — an inert addition sitting disabled isn't a change from
            // the saved revision's point of view.
            None if enabled => plus += 1,
            _ => {}
        }
    }
    Ok(DirtySummary { plus, minus })
}

/// Step 1 of §5's cycle: a readable, not-yet-runnable description of an
/// intended mutation against one loadout. Nothing has happened yet — not
/// even a snapshot — until a caller calls `approve()`.
pub struct Plan<'a> {
    core: &'a Core,
    loadout_id: i64,
    kind: String,
    steps: Vec<String>,
}

impl<'a> Plan<'a> {
    pub fn new(core: &'a Core, loadout_id: i64, kind: impl Into<String>) -> Self {
        Self {
            core,
            loadout_id,
            kind: kind.into(),
            steps: Vec::new(),
        }
    }

    /// Record one step the plan intends to take, in whatever order a caller
    /// wants them shown.
    pub fn step(&mut self, description: impl Into<String>) -> &mut Self {
        self.steps.push(description.into());
        self
    }

    /// What §5 calls "PLAN (lisible)" — the steps as a client would show a
    /// human, or hand to a policy engine, before deciding to approve.
    #[must_use]
    pub fn describe(&self) -> &[String] {
        &self.steps
    }

    /// "APPROBATION" — a deliberate, separate call. `apply` exists only on
    /// the type this returns, so no caller can reach SNAPSHOT/APPLY without
    /// passing through here. Core does not decide *when* this should be
    /// called or by whom (§7.2) — it only refuses to skip it.
    #[must_use]
    pub fn approve(self) -> ApprovedPlan<'a> {
        ApprovedPlan {
            core: self.core,
            loadout_id: self.loadout_id,
            kind: self.kind,
            steps: self.steps,
        }
    }
}

/// Step 2 of §5's cycle onward — a `Plan` a caller has committed to running.
pub struct ApprovedPlan<'a> {
    core: &'a Core,
    loadout_id: i64,
    kind: String,
    steps: Vec<String>,
}

impl<'a> ApprovedPlan<'a> {
    /// SNAPSHOT → APPLY → VÉRIFICATION → COMMIT | ROLLBACK auto.
    ///
    /// `mutate` performs the actual write and returns whatever the caller
    /// needs back (e.g. "was a row actually updated"). `verify` receives
    /// that value and must confirm the mutation actually holds — a re-read
    /// of the loadout that checks it matches what was asked for, not just
    /// "`mutate` didn't throw". If either `mutate` or `verify` fails, the
    /// loadout is restored to its state from right before this call and the
    /// error is returned — nothing is committed. If both succeed, one
    /// `transaction.committed` event is journaled.
    ///
    /// The SNAPSHOT this step name refers to is deliberately **not** the
    /// public, revision-facing `snapshot_loadout`/`rollback_loadout` pair —
    /// it is an in-memory copy of the loadout's state, taken and (if needed)
    /// restored without touching the event log at all. The two had to be
    /// split apart: every `apply()` call — meaning every mod toggle — was
    /// journaling a `loadout.snapshot` event, which flooded the visible
    /// Journal with one entry per click and inflated `revision_count` on
    /// every toggle instead of only when a human asks to save a revision.
    /// The public pair stays exactly what `ActiveInstanceCard`'s "Enregistrer"
    /// / "Annuler les modifications" call, and this private safety net never
    /// shows up next to it.
    pub fn apply<T>(
        self,
        mutate: impl FnOnce(&Core) -> Result<T>,
        verify: impl FnOnce(&Core, &T) -> Result<()>,
    ) -> Result<T> {
        let before = self.core.select_loadout_state(self.loadout_id)?;

        match mutate(self.core).and_then(|value| verify(self.core, &value).map(|()| value)) {
            Ok(value) => {
                let mut tx = Transaction::new(self.core, self.kind);
                for step in &self.steps {
                    tx.step(step.clone());
                }
                tx.commit()?;
                Ok(value)
            }
            Err(err) => {
                // Best-effort: if the restore itself also fails, the
                // mutation's own error is still the one the caller asked
                // about — surfacing that failure instead would hide why
                // this was being undone in the first place.
                let _ = self.core.replace_loadout_entries(self.loadout_id, &before);
                Err(err)
            }
        }
    }
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
    /// Which loadout this entry is about — only known for `loadout.snapshot`
    /// / `loadout.rollback`, whose payload structurally carries it. `None`
    /// for `transaction.committed` entries: their payload is a free-text
    /// step list (e.g. "mod 3 in loadout 1 → enabled"), not a loadout id
    /// field, so a client filtering "this loadout's revisions" must expect
    /// gaps rather than assume every entry can be attributed.
    pub loadout_id: Option<i64>,
}

const JOURNALED_KINDS: [&str; 3] = ["transaction.committed", SNAPSHOT_EVENT, ROLLBACK_EVENT];

/// The recent journal, newest first.
pub fn list(core: &Core, limit: usize) -> Result<Vec<TransactionRecord>> {
    let events = core.select_events(&JOURNALED_KINDS, limit)?;
    Ok(events
        .into_iter()
        .map(|(id, occurred_at, kind, payload)| {
            let (label, steps, loadout_id) = match kind.as_str() {
                "transaction.committed" => {
                    #[derive(serde::Deserialize)]
                    struct Committed {
                        kind: String,
                        #[serde(default)]
                        steps: Vec<String>,
                    }
                    match serde_json::from_str::<Committed>(&payload) {
                        Ok(c) => (c.kind, c.steps, None),
                        Err(_) => (kind.clone(), vec![payload], None),
                    }
                }
                SNAPSHOT_EVENT => match serde_json::from_str::<SnapshotPayloadOwned>(&payload) {
                    Ok(s) => (
                        kind.clone(),
                        vec![format!(
                            "{} entrée(s) d'instance capturée(s)",
                            s.entries.len()
                        )],
                        Some(s.loadout_id),
                    ),
                    Err(_) => (kind.clone(), vec![payload], None),
                },
                _ => match serde_json::from_str::<RollbackPayload>(&payload) {
                    Ok(r) => (
                        kind.clone(),
                        vec![format!(
                            "{} entrée(s) restaurée(s) depuis l'instantané #{}",
                            r.restored, r.restored_from
                        )],
                        Some(r.loadout_id),
                    ),
                    Err(_) => (kind.clone(), vec![payload], None),
                },
            };
            TransactionRecord {
                id,
                occurred_at,
                kind: label,
                steps,
                loadout_id,
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
                    version: None,
                },
                crate::loadout::ImportEntry {
                    name: "B".into(),
                    source_path: dir.clone(),
                    enabled: true,
                    version: None,
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
        assert_eq!(
            first.loadout_id,
            Some(lo.id),
            "a client filtering \"this loadout's revisions\" needs this populated"
        );
        let snapshot_entry = journal
            .iter()
            .find(|r| r.kind == SNAPSHOT_EVENT)
            .expect("the snapshot is in the journal too");
        assert_eq!(snapshot_entry.loadout_id, Some(lo.id));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn committed_entries_carry_no_loadout_id_rather_than_a_guess() {
        let core = Core::open_in_memory().unwrap();
        crate::loadout::create(&core, "skyrimse", "Test").unwrap();
        let journal = list(&core, 10).unwrap();
        let created = journal
            .iter()
            .find(|r| r.kind == "loadout.create")
            .expect("loadout::create journals under its own kind");
        assert_eq!(
            created.loadout_id, None,
            "transaction.committed's payload has no structural loadout id field"
        );
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

    fn loadout_with_one_mod(core: &Core) -> (i64, i64) {
        let lo = crate::loadout::create(core, "skyrimse", "Test").unwrap();
        crate::loadout::import_mods(
            core,
            lo.id,
            &[crate::loadout::ImportEntry {
                name: "A".into(),
                source_path: "F:/mods/A".into(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();
        let mod_id = crate::loadout::entries(core, lo.id).unwrap()[0].mod_id;
        (lo.id, mod_id)
    }

    #[test]
    fn apply_commits_exactly_one_event_on_success_and_no_public_snapshot() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, mod_id) = loadout_with_one_mod(&core);
        let before = core.event_count().unwrap();

        let mut plan = Plan::new(&core, loadout_id, "test.mutate");
        plan.step("disable the mod");
        let updated = plan
            .approve()
            .apply(
                |core| core.set_loadout_entry_enabled(loadout_id, mod_id, false),
                |_core, updated| {
                    assert!(*updated, "verify sees the value mutate returned");
                    Ok(())
                },
            )
            .unwrap();

        assert!(updated);
        assert!(!crate::loadout::entries(&core, loadout_id).unwrap()[0].enabled);

        assert_eq!(
            core.event_count().unwrap(),
            before + 1,
            "exactly the commit — no public snapshot event alongside it"
        );
        let journal = list(&core, 10).unwrap();
        assert!(
            !journal.iter().any(|r| r.kind == SNAPSHOT_EVENT),
            "apply's internal safety capture must never appear as a public \
             loadout.snapshot — that would inflate revision_count on every toggle"
        );
        assert!(
            journal.iter().any(|r| r.kind == "test.mutate"),
            "the commit is journaled under the plan's own kind, not a generic label"
        );
    }

    /// `mutate` here really does write to the database — this proves `apply`
    /// undoes a mutation that *succeeded*, not just one that never ran, the
    /// moment `verify` refuses it.
    #[test]
    fn apply_restores_state_and_never_commits_when_verify_refuses() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, mod_id) = loadout_with_one_mod(&core);
        let before = core.event_count().unwrap();

        let mut plan = Plan::new(&core, loadout_id, "test.mutate");
        plan.step("disable the mod, but verification will refuse it");
        let result: Result<()> = plan.approve().apply(
            |core| {
                core.set_loadout_entry_enabled(loadout_id, mod_id, false)?;
                Ok(())
            },
            |_core, _| -> Result<()> {
                Err(crate::CoreError::Message(
                    "verification refuses on purpose".into(),
                ))
            },
        );

        assert!(result.is_err());
        assert!(
            crate::loadout::entries(&core, loadout_id).unwrap()[0].enabled,
            "the mutation was undone: the mod is back to enabled, not stuck disabled"
        );

        let journal = list(&core, 10).unwrap();
        assert!(
            !journal
                .iter()
                .any(|r| r.kind == SNAPSHOT_EVENT || r.kind == ROLLBACK_EVENT),
            "the internal safety restore never touches the public snapshot/rollback \
             vocabulary — it would otherwise show up in the Journal and in revision_count"
        );
        assert!(
            !journal.iter().any(|r| r.kind == "test.mutate"),
            "a refused verification must never leave a committed event behind"
        );
        assert_eq!(
            core.event_count().unwrap(),
            before,
            "a refused verification journals nothing at all"
        );
    }

    #[test]
    fn revision_count_tracks_every_snapshot_taken() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, _) = loadout_with_one_mod(&core);

        assert_eq!(revision_count(&core, loadout_id).unwrap(), 0);
        snapshot_loadout(&core, loadout_id).unwrap();
        assert_eq!(revision_count(&core, loadout_id).unwrap(), 1);
        snapshot_loadout(&core, loadout_id).unwrap();
        snapshot_loadout(&core, loadout_id).unwrap();
        assert_eq!(revision_count(&core, loadout_id).unwrap(), 3);
    }

    #[test]
    fn revision_count_never_mixes_up_two_loadouts() {
        let core = Core::open_in_memory().unwrap();
        let (a, _) = loadout_with_one_mod(&core);
        let (b, _) = loadout_with_one_mod(&core);

        snapshot_loadout(&core, a).unwrap();
        snapshot_loadout(&core, a).unwrap();
        snapshot_loadout(&core, b).unwrap();

        assert_eq!(revision_count(&core, a).unwrap(), 2);
        assert_eq!(revision_count(&core, b).unwrap(), 1);
    }

    #[test]
    fn dirty_summary_is_clean_before_any_snapshot_exists() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, _) = loadout_with_one_mod(&core);
        assert!(dirty_summary(&core, loadout_id).unwrap().is_clean());
    }

    #[test]
    fn dirty_summary_reports_plus_and_minus_against_the_latest_snapshot() {
        let core = Core::open_in_memory().unwrap();
        let lo = crate::loadout::create(&core, "skyrimse", "Test").unwrap();
        crate::loadout::import_mods(
            &core,
            lo.id,
            &[
                crate::loadout::ImportEntry {
                    name: "A".into(),
                    source_path: "F:/mods/A".into(),
                    enabled: true,
                    version: None,
                },
                crate::loadout::ImportEntry {
                    name: "B".into(),
                    source_path: "F:/mods/B".into(),
                    enabled: false,
                    version: None,
                },
            ],
        )
        .unwrap();
        snapshot_loadout(&core, lo.id).unwrap();
        assert!(
            dirty_summary(&core, lo.id).unwrap().is_clean(),
            "right after a snapshot, nothing has changed yet"
        );

        let rows = crate::loadout::entries(&core, lo.id).unwrap();
        let a_id = rows.iter().find(|r| r.name == "A").unwrap().mod_id;
        let b_id = rows.iter().find(|r| r.name == "B").unwrap().mod_id;
        crate::loadout::set_enabled(&core, lo.id, a_id, false).unwrap();
        crate::loadout::set_enabled(&core, lo.id, b_id, true).unwrap();

        let dirty = dirty_summary(&core, lo.id).unwrap();
        assert_eq!(dirty.plus, 1, "B turned on");
        assert_eq!(dirty.minus, 1, "A turned off");
        assert!(!dirty.is_clean());

        rollback_loadout(&core, lo.id).unwrap();
        assert!(
            dirty_summary(&core, lo.id).unwrap().is_clean(),
            "rollback restores the snapshot's own state, which is clean by definition"
        );
    }

    #[test]
    fn dirty_summary_counts_a_mod_imported_after_the_snapshot_only_if_enabled() {
        let core = Core::open_in_memory().unwrap();
        let lo = crate::loadout::create(&core, "skyrimse", "Test").unwrap();
        crate::loadout::import_mods(
            &core,
            lo.id,
            &[crate::loadout::ImportEntry {
                name: "A".into(),
                source_path: "F:/mods/A".into(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();
        snapshot_loadout(&core, lo.id).unwrap();

        crate::loadout::import_mods(
            &core,
            lo.id,
            &[
                crate::loadout::ImportEntry {
                    name: "B (on)".into(),
                    source_path: "F:/mods/B".into(),
                    enabled: true,
                    version: None,
                },
                crate::loadout::ImportEntry {
                    name: "C (off)".into(),
                    source_path: "F:/mods/C".into(),
                    enabled: false,
                    version: None,
                },
            ],
        )
        .unwrap();

        let dirty = dirty_summary(&core, lo.id).unwrap();
        assert_eq!(dirty.plus, 1, "only the enabled newcomer counts");
        assert_eq!(dirty.minus, 0);
    }
}
