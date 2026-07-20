//! The template cache evictor (context C6): the self-supervised loop that drops a removed
//! project's cached template rows.
//!
//! [`Templates`] caches each `(kind, scope)`'s rows and invalidates them on its own writes — which
//! is coherent only while it is the single writer. Removing a project is the exception: the store
//! cascades that project's template rows away without this aggregate seeing a write, so its cached
//! entries would otherwise describe a project that is gone. This evictor closes that gap by
//! consuming [`DomainEvent::ProjectRemoved`] from the bus and dropping the removed project's
//! entries, keeping "a tool cannot reach another project's data" true of the cache and not only of
//! the store.
//!
//! It holds a [`Weak`] reference to the aggregate rather than the supervisor — the cache is the
//! only thing it needs alive — and no sender, so the loop ends when the façade drops the bus (app
//! shutdown). Self-supervised like the samplers: a panicking pass is isolated and the loop
//! restarts. Because a restart resubscribes at the live tail, and a lagged subscriber has a gap
//! either way, both paths fall back to dropping the whole cache — a re-read is cheap, a stale
//! entry is not.

use std::sync::{Arc, Weak};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::events::{DomainEvent, EventBus};
use crate::ports::Clock;
use crate::supervision::supervise;

use super::template::Templates;

/// Drops a removed project's cached template rows. Built once by the composition root (via
/// [`crate::facade::Facade::template_eviction_loop`]) and spawned on the runtime.
pub struct TemplateEvictor {
    templates: Weak<Templates>,
    events: broadcast::Receiver<DomainEvent>,
    clock: Arc<dyn Clock>,
}

impl TemplateEvictor {
    /// Builds an evictor over the template aggregate and the event bus, holding the aggregate
    /// weakly (so it never keeps the app alive) and keeping no sender of its own (so the loop ends
    /// when the bus closes).
    pub fn new(templates: &Arc<Templates>, bus: &EventBus, clock: Arc<dyn Clock>) -> Self {
        Self {
            templates: Arc::downgrade(templates),
            events: bus.subscribe(),
            clock,
        }
    }

    /// Runs until the bus closes (app shutdown) or the aggregate is dropped, supervising the inner
    /// loop so a panicking pass is isolated and restarted (see [`supervise`]). Returned for the
    /// composition root to spawn once on its runtime.
    pub async fn run(self) {
        let clock = self.clock.clone();
        supervise(clock, move || self.fork().evict_loop()).await;
    }

    /// A copy of the evictor reading the same bus from the live tail, for each restart of the inner
    /// loop. Resubscribing needs no sender, so a fork cannot keep the bus open past shutdown.
    fn fork(&self) -> Self {
        Self {
            templates: self.templates.clone(),
            events: self.events.resubscribe(),
            clock: self.clock.clone(),
        }
    }

    /// The eviction loop: drop everything cached, then drop each removed project's entries as the
    /// removals arrive. Starts cold because a restart resumes at the live tail, so a removal
    /// announced while the loop was down would otherwise be missed forever.
    async fn evict_loop(mut self) {
        let Some(templates) = self.templates.upgrade() else {
            return;
        };
        templates.forget_all();
        drop(templates);
        loop {
            let received = self.events.recv().await;
            let Some(templates) = self.templates.upgrade() else {
                return;
            };
            match received {
                Err(RecvError::Closed) => return,
                // The gap may have hidden a removal, so nothing cached can be trusted.
                Err(RecvError::Lagged(_)) => templates.forget_all(),
                Ok(DomainEvent::ProjectRemoved { id }) => templates.forget_project(id),
                Ok(_) => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "template_evictor_tests.rs"]
mod tests;
