//! Machine capacity shared by hosts, epics and both implementations
//! (`packages/toolu-core/src/resources`, #421): state, policy and lock,
//! pressure holds, leases, worktree bindings and the managed job a ledger
//! check runs under. The epic engine (#434) adds agent migration here.

/// A worktree's resource binding.
pub mod binding;
/// A command under a job lease.
pub mod jobs;
/// Admission, patches, release and reconciliation.
pub mod leases;
/// The resource lock and atomic JSON writes.
pub mod lock;
/// Sustained-pressure decisions.
pub mod pressure;
/// Machine sampling.
pub mod sample;
/// The resource state and policy.
pub mod store;
