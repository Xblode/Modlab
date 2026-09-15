//! Loadouts — a named, versionable mod profile.
//! See docs/01-architecture-decidee.md §5. This is a first slice: creating
//! a named loadout and journaling it. Revisions (immutable snapshots),
//! mods-in-a-loadout, and load order are real domain work for a later
//! pass, once the CAS/transaction pieces they depend on are solid.

use crate::transaction::Transaction;
use crate::{Core, Result};

pub struct Loadout {
    pub name: String,
}

/// Create a new, empty loadout and journal it as a transaction.
pub fn create(core: &Core, name: &str) -> Result<Loadout> {
    let mut tx = Transaction::new(core, "loadout.create");
    tx.step(format!("created empty loadout '{name}'"));
    tx.commit()?;

    Ok(Loadout {
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_a_loadout_journals_a_transaction() {
        let core = Core::open_in_memory().unwrap();
        let before = core.event_count().unwrap();

        let loadout = create(&core, "Main 2026").unwrap();

        assert_eq!(loadout.name, "Main 2026");
        assert_eq!(core.event_count().unwrap(), before + 1);
    }
}
