//! FocusWall Core Library
//!
//! Pure business logic, policy structures, scheduling calculations,
//! domain normalization, persistent storage, configuration, DNS management,
//! domain IP resolution, and nftables firewall backstop.

pub mod config;
pub mod dns;
pub mod domain;
pub mod firewall;
pub mod policy;
pub mod resolver;
pub mod schedule;
pub mod storage;

pub use config::*;
pub use dns::*;
pub use domain::*;
pub use firewall::*;
pub use policy::*;
pub use resolver::*;
pub use schedule::*;
pub use storage::*;
