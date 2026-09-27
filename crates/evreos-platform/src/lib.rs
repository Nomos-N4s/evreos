//! Platform services, reached through safe bindings.
//!
//! The shell reaches its platform services through this crate:
//! default-browser registration, and later the secure credential store,
//! update verification and local rollout evaluation. Windowing, the
//! chrome's accessibility and the engine reach the operating system through
//! crates of their own. The crate holds no `unsafe` of its own. Each service
//! goes through a safe binding kept to the platform that needs it by a
//! target-specific dependency table, and the change that adds a binding
//! states its byte cost against `budgets.toml`.
#![forbid(unsafe_code)]

pub mod default_browser;
pub mod update;
