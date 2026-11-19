//! A JSON-RPC client for chain nodes and explorers (stage 2, spec E1).
//!
//! What it guarantees, beyond "sends requests":
//!
//! * **A provider cannot link the owner's addresses.** A batch belongs to one
//!   *subject* (an address, or any other privacy scope): [`RpcClient::batch`]
//!   takes the subject once and sends only that subject's calls in one HTTP
//!   request. Requests of *different* subjects to the same host are never
//!   merged and are separated in time by a randomized pause
//!   ([`RpcConfig::subject_gap_min`]..[`RpcConfig::subject_gap_max`]), so they
//!   do not arrive as one burst from one IP.
//! * **A polite client.** One request at a time per host with a minimum
//!   interval, retries with exponential backoff on throttling (429), server
//!   errors and network failures, `Retry-After` honoured, a timeout on every
//!   request.
//! * **No plain HTTP.** Endpoints must be `https`; the only exception is a
//!   loopback address when [`RpcConfig::allow_loopback_http`] is set, which is
//!   how the tests reach a local stub server.

mod client;
mod limiter;
mod port;

pub use client::{RpcCall, RpcClient, RpcConfig};
pub use limiter::HostLimiter;

#[cfg(test)]
mod tests;
