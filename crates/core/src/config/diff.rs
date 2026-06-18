//! Diffing two `solo.yml` snapshots into an add/update/remove/rename change set.

use serde::Serialize;

use super::model::SoloYml;

/// A rename: the same command string moved from one process name to another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

/// The difference between a previous and current config, by process name. Carried
/// to adapters in [`crate::events::DomainEvent::ConfigChanged`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ConfigSync {
    /// Newly added process names.
    pub added: Vec<String>,
    /// Process names whose spec changed in place (any field).
    pub updated: Vec<String>,
    /// Removed process names.
    pub removed: Vec<String>,
    /// Unambiguous renames (a removed/added pair sharing one command string).
    pub renamed: Vec<Rename>,
}

impl ConfigSync {
    /// True when nothing changed between the two snapshots.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.renamed.is_empty()
    }
}

/// Computes the change set from `old` to `new`.
///
/// Rename detection runs first: a name present only on the removed side and one
/// present only on the added side that share the **same command string** — with
/// that command string unique on both sides (unambiguous) — are paired as a rename
/// instead of a remove + add. Whatever is not a rename is then classified as
/// added, removed, or (for names in both) updated when the spec differs.
pub fn diff(old: &SoloYml, new: &SoloYml) -> ConfigSync {
    let mut removed: Vec<String> = old
        .processes
        .keys()
        .filter(|name| !new.processes.contains_key(*name))
        .cloned()
        .collect();
    let mut added: Vec<String> = new
        .processes
        .keys()
        .filter(|name| !old.processes.contains_key(*name))
        .cloned()
        .collect();

    let renamed = detect_renames(old, new, &removed, &added);
    removed.retain(|name| !renamed.iter().any(|r| &r.from == name));
    added.retain(|name| !renamed.iter().any(|r| &r.to == name));

    let updated: Vec<String> = new
        .processes
        .iter()
        .filter_map(|(name, spec)| match old.processes.get(name) {
            Some(prev) if prev != spec => Some(name.clone()),
            _ => None,
        })
        .collect();

    ConfigSync {
        added,
        updated,
        removed,
        renamed,
    }
}

fn detect_renames(
    old: &SoloYml,
    new: &SoloYml,
    removed: &[String],
    added: &[String],
) -> Vec<Rename> {
    let mut renames = Vec::new();
    for from in removed {
        let Some(command) = old.processes.get(from).map(|spec| &spec.command) else {
            continue;
        };

        // Unambiguous: exactly one removed-side and one added-side name carry this
        // command string. Otherwise it is a genuine remove + add, not a rename.
        let removed_matches = removed
            .iter()
            .filter(|name| {
                old.processes
                    .get(*name)
                    .is_some_and(|s| &s.command == command)
            })
            .count();
        let mut added_matches = added.iter().filter(|name| {
            new.processes
                .get(*name)
                .is_some_and(|s| &s.command == command)
        });

        if removed_matches == 1 {
            if let Some(to) = added_matches.next() {
                if added_matches.next().is_none() {
                    renames.push(Rename {
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
            }
        }
    }
    renames
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;

    fn config(yaml: &str) -> SoloYml {
        parse(yaml).expect("test config parses")
    }

    #[test]
    fn detects_add_update_remove() {
        let old = config(
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
        );
        let new = config("processes:\n  Web:\n    command: npm run start\n  Worker:\n    command: cargo run --bin worker\n");
        let d = diff(&old, &new);
        assert_eq!(d.added, vec!["Worker"]);
        assert_eq!(d.removed, vec!["Api"]);
        assert_eq!(d.updated, vec!["Web"]); // command changed
    }

    #[test]
    fn unchanged_config_is_empty_diff() {
        let yaml = "processes:\n  Web:\n    command: npm run dev\n";
        assert!(diff(&config(yaml), &config(yaml)).is_empty());
    }

    #[test]
    fn unambiguous_rename_is_paired_not_add_remove() {
        let old = config("processes:\n  Web:\n    command: npm run dev\n");
        let new = config("processes:\n  Frontend:\n    command: npm run dev\n");
        let d = diff(&old, &new);
        assert!(d.added.is_empty());
        assert!(d.removed.is_empty());
        assert!(d.updated.is_empty());
        assert_eq!(
            d.renamed,
            vec![Rename {
                from: "Web".into(),
                to: "Frontend".into()
            }]
        );
    }

    #[test]
    fn ambiguous_same_command_is_not_treated_as_rename() {
        // Two removed and two added share the command string → ambiguous, so each
        // is reported as a plain add/remove rather than a guessed pairing.
        let old = config("processes:\n  A:\n    command: run\n  B:\n    command: run\n");
        let new = config("processes:\n  C:\n    command: run\n  D:\n    command: run\n");
        let d = diff(&old, &new);
        assert!(d.renamed.is_empty());
        assert_eq!(d.added.len(), 2);
        assert_eq!(d.removed.len(), 2);
    }
}
