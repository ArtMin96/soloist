//! Network port discovery (context C5): which TCP ports each managed process is listening
//! on.
//!
//! Like the metrics domain, this context owns *how port discovery works* — the OS read it
//! depends on ([`PortProbe`], the port it defines for itself) and the policy that drives it
//! ([`PortScanner`]). The OS read is an adapter (`crates/sys`, over `/proc`); a missing
//! adapter degrades to [`NoopPortProbe`]. Discovered ports surface on
//! [`crate::process::ProcessView::ports`].

mod probe;
mod scanner;
mod waiter;

#[cfg(test)]
mod test_support;

pub use probe::{NoopPortProbe, PortProbe};
pub use scanner::PortScanner;
pub use waiter::{wait_for_port, WaitForPortError};
