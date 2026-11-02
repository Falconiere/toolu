//! One relay authority, durable submissions and recoverable worker attempts.

/// Controller delivery with a durable outbox.
pub mod client;
/// Persistent global machine registrations.
pub mod config;
/// Serialized worker scheduling and recovery.
pub mod driver;
mod durable;
/// Replace native relay/controller state only after syncing its data and directory.
pub use durable::write as write_state;
/// Authenticated Herdr request receiver.
pub mod rpc;
/// Registered worker connections and capacity.
pub mod status;
/// Transactional task and capacity ledger.
pub mod store;
mod worker;
