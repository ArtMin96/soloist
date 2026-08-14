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
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{StoreError, TrustGrant, TrustRepo};

mod releaser;
mod requests;

pub use requests::{PendingTrustRequest, TrustRequestSubmission, TrustRequests};

/// Whether a command variant is trusted to run. Trust is per command *variant*,
/// identified by its [`Hash`] over command/working_dir/env (see
/// [`ProcessSpec::variant_hash`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trust {
    /// The variant has not been trusted (or was invalidated by an edit).
    Untrusted,
    /// The variant is trusted; the key that was matched is carried for reference.
    Trusted { variant_hash: Hash },
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
            Ok(Trust::Trusted {
                variant_hash: variant,
            })
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

    /// Trusts `spec`'s variant within `project`, recording that a process asked for it and why.
    ///
    /// The grant itself is the same row an ordinary [`trust`](Self::trust) writes — it has to be,
    /// since the start gate consults it on every start, auto-start, crash restart and file-watch
    /// restart, and a second kind of trust in that path would be a second thing to get wrong. The
    /// provenance is what separates a grant an agent asked for from one the user authored, so the
    /// user can review and take back what they approved on someone else's behalf.
    pub fn trust_requested(
        &self,
        project: ProjectId,
        spec: &ProcessSpec,
        requested_by: ProcessId,
        reason: &str,
        granted_at_unix_millis: u64,
    ) -> Result<(), StoreError> {
        self.repo.set_trusted_with_provenance(
            project,
            &spec.variant_hash(),
            requested_by,
            reason,
            granted_at_unix_millis,
        )
    }

    /// Every trusted command variant in `project`, with the provenance of each — what the review
    /// surface lists so a grant can be taken back.
    pub fn grants(&self, project: ProjectId) -> Result<Vec<TrustGrant>, StoreError> {
        self.repo.list_grants(project)
    }

    /// Revokes trust for `spec`'s variant within `project`.
    pub fn untrust(&self, project: ProjectId, spec: &ProcessSpec) -> Result<(), StoreError> {
        self.repo.revoke(project, &spec.variant_hash())
    }

    /// Revokes a variant named by its own key, as the review list names it — there is no spec to
    /// resolve for a grant an agent asked for, so the key is the handle.
    pub fn untrust_variant(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        self.repo.revoke(project, variant)
    }

    /// Whether the user has authorised Soloist to make changes within `project` — the gate the
    /// version-control write side spends. Coarser than command trust on purpose: it authorises
    /// Soloist to act on the project rather than one command line to run.
    pub fn is_project_trusted(&self, project: ProjectId) -> Result<bool, StoreError> {
        self.repo.is_project_trusted(project)
    }

    /// Records that authorisation for `project`.
    pub fn trust_project(&self, project: ProjectId) -> Result<(), StoreError> {
        self.repo.set_project_trusted(project)
    }

    /// Withdraws it again.
    pub fn untrust_project(&self, project: ProjectId) -> Result<(), StoreError> {
        self.repo.revoke_project(project)
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
