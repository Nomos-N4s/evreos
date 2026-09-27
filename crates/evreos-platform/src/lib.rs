//! Platform services, reached through safe bindings.
//!
//! The shell reaches the operating system through this crate, and only
//! through it: default-browser registration, and later the secure credential
//! store, update verification and local rollout evaluation. The crate holds no
//! `unsafe` of its own. Each service goes through a safe binding kept to the
//! platform that needs it by a target-specific dependency table, and the
//! change that adds a binding states its byte cost against `budgets.toml`.
#![forbid(unsafe_code)]
