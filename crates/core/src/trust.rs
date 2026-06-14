//! Trust evaluation over the durable [`TrustRepo`].
//!
//! Trust is a security boundary enforced in the core (never in the UI): a command
//! may start/auto-start/restart only when its exact variant is trusted within its
//! project. This module turns a [`ProcessSpec`] into its variant key and asks the
//! durable store. The actual gating at start time lands with the supervisor, but
//! the decision lives here so every adapter funnels through one place.

use std::sync::Arc;

use crate::config::ProcessSpec;
use crate::hash::Hash;
use crate::ids::ProjectId;
use crate::ports::{StoreError, TrustRepo};

/// Whether a command variant is trusted to run. Trust is per command *variant*,
/// identified by its [`Hash`] over command/working_dir/env (see
/// [`ProcessSpec::variant_hash`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trust {
    /// The variant has not been trusted (or was invalidated by an edit).
    Untrusted,
    /// The variant is trusted; the key that was matched is carried for reference.
    Trusted { variant: Hash },
}

/// The trust gate over the durable store.
pub struct TrustStore {
    repo: Arc<dyn TrustRepo>,
}

impl TrustStore {
    /// Builds a trust gate over the durable trust repository.
    pub fn new(repo: Arc<dyn TrustRepo>) -> Self {
        Self { repo }
    }

    /// The trust status of `spec` within `project`.
    pub fn status(&self, project: ProjectId, spec: &ProcessSpec) -> Result<Trust, StoreError> {
        let variant = spec.variant_hash();
        if self.repo.is_trusted(project, &variant)? {
            Ok(Trust::Trusted { variant })
        } else {
            Ok(Trust::Untrusted)
        }
    }

    /// Boolean convenience over [`TrustStore::status`] — what the start gate asks.
    pub fn is_trusted(&self, project: ProjectId, spec: &ProcessSpec) -> Result<bool, StoreError> {
        Ok(matches!(self.status(project, spec)?, Trust::Trusted { .. }))
    }

    /// Trusts `spec`'s variant within `project`.
    pub fn trust(&self, project: ProjectId, spec: &ProcessSpec) -> Result<(), StoreError> {
        self.repo.set_trusted(project, &spec.variant_hash())
    }

    /// Revokes trust for `spec`'s variant within `project`.
    pub fn untrust(&self, project: ProjectId, spec: &ProcessSpec) -> Result<(), StoreError> {
        self.repo.revoke(project, &spec.variant_hash())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeTrustRepo;
    use std::collections::BTreeMap;

    fn spec(command: &str) -> ProcessSpec {
        ProcessSpec {
            command: command.to_string(),
            working_dir: None,
            auto_start: true,
            auto_restart: false,
            restart_when_changed: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn editing_invalidates_trust_while_renaming_preserves_it() {
        let store = TrustStore::new(Arc::new(FakeTrustRepo::new()));
        let project = ProjectId::from_raw(1);
        let web = spec("npm run dev");

        assert!(!store.is_trusted(project, &web).unwrap());
        store.trust(project, &web).unwrap();
        assert!(store.is_trusted(project, &web).unwrap());

        // A rename keeps command/working_dir/env, so the variant — and its trust —
        // is unchanged (the name is not part of the spec).
        assert!(store.is_trusted(project, &spec("npm run dev")).unwrap());

        // Editing the command yields a new variant that is not trusted.
        assert!(!store.is_trusted(project, &spec("npm run start")).unwrap());

        // Editing the environment also invalidates trust.
        let mut env_changed = web.clone();
        env_changed.env.insert("PORT".into(), "3000".into());
        assert!(!store.is_trusted(project, &env_changed).unwrap());
    }

    #[test]
    fn untrust_revokes() {
        let store = TrustStore::new(Arc::new(FakeTrustRepo::new()));
        let project = ProjectId::from_raw(7);
        let web = spec("npm run dev");
        store.trust(project, &web).unwrap();
        assert!(store.is_trusted(project, &web).unwrap());
        store.untrust(project, &web).unwrap();
        assert!(!store.is_trusted(project, &web).unwrap());
    }

    #[test]
    fn trust_is_scoped_per_project() {
        let store = TrustStore::new(Arc::new(FakeTrustRepo::new()));
        let web = spec("npm run dev");
        store.trust(ProjectId::from_raw(1), &web).unwrap();
        assert!(store.is_trusted(ProjectId::from_raw(1), &web).unwrap());
        assert!(
            !store.is_trusted(ProjectId::from_raw(2), &web).unwrap(),
            "trusting in one project must not trust another"
        );
    }
}
