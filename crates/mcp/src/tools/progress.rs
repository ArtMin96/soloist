//! Telling a client what a long tool call is doing while it is still doing it.
//!
//! The protocol makes this the client's choice: it puts a progress token on the request it wants to
//! hear about, and a server "**MAY** then send progress notifications" against that token. A request
//! that carries no token is answered exactly as it always was — nothing here runs for it, and
//! nothing about it changes on the wire.
//!
//! What a client does with a notification is its own business. The specification permits rather than
//! requires the useful part: an implementation "**MAY** choose to reset the timeout clock when
//! receiving a progress notification … as this implies that work is actually happening". A client
//! that does keeps a long operation alive; one that does not still learns where the operation has
//! got to, which is worth having on its own.

use rmcp::handler::server::common::{AsRequestContext, FromContextPart};
use rmcp::model::{ErrorData, ProgressNotificationParam, ProgressToken};
use rmcp::service::{Peer, RoleServer};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// How many remarks may be waiting to become notifications before the newest are dropped.
///
/// A remark is worth having only while it is current, so falling behind is answered by dropping
/// rather than by queueing — and the operation being described is never slowed by whoever is
/// listening. Small on purpose: the app already coalesces and rate-limits at the source, so a
/// backlog past a handful means the host is not reading its own stream.
const REPORT_BACKLOG: usize = 8;

/// Whether the caller asked to be told what its call is doing, and what to tell it against.
///
/// Taken as a tool handler's own argument rather than read from somewhere ambient, so a call that
/// nobody asked about is a value a caller — or a test — can simply hold ([`Reporting::unasked`]),
/// and the two paths through a tool are told apart by the type rather than by a condition.
pub(crate) struct Reporting(Option<(Peer<RoleServer>, ProgressToken)>);

impl Reporting {
    /// Nobody asked. What a request carrying no progress token produces.
    pub(crate) fn unasked() -> Self {
        Self(None)
    }

    /// Starts turning remarks into `notifications/progress`, or answers `None` when nobody asked.
    ///
    /// The forwarding ends when the returned sender is dropped, which is what the call finishing
    /// does — so awaiting the handle after the call is what lets the last remark through and stops
    /// the forwarding afterwards, as the protocol requires ("progress notifications **MUST** stop
    /// after completion").
    ///
    /// The count carried as `progress` only ever rises, which the protocol requires of it ("the
    /// `progress` value **MUST** increase with each notification, even if the total is unknown"). No
    /// total is sent, because there is none to know: what an exchange with a remote has left to do
    /// is not something anything here can say, and the protocol allows omitting it rather than
    /// inventing one. The remark itself travels as the human-readable `message`.
    pub(crate) fn forwarding(self) -> Option<(mpsc::Sender<String>, JoinHandle<()>)> {
        let (peer, token) = self.0?;
        let (reports, mut reported) = mpsc::channel(REPORT_BACKLOG);
        let forwarding = tokio::spawn(async move {
            let mut made = 0.0;
            while let Some(note) = reported.recv().await {
                made += 1.0;
                let told = peer
                    .notify_progress(
                        ProgressNotificationParam::new(token.clone(), made).with_message(note),
                    )
                    .await;
                // A host that has stopped listening is not a failure of the operation being
                // described, so the forwarding ends quietly and the call it belongs to carries on.
                if told.is_err() {
                    break;
                }
            }
        });
        Some((reports, forwarding))
    }
}

impl<C> FromContextPart<C> for Reporting
where
    C: AsRequestContext,
{
    fn from_context_part(context: &mut C) -> Result<Self, ErrorData> {
        let context = context.as_request_context();
        Ok(match context.meta.get_progress_token() {
            Some(token) => Self(Some((context.peer.clone(), token))),
            None => Self::unasked(),
        })
    }
}
