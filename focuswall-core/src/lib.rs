//! FocusWall Core Library
//!
//! Pure business logic, policy structures, scheduling calculations,
//! domain normalization, and state evaluations without needing root privileges.

pub mod domain;
pub mod policy;
pub mod schedule;

pub use domain::*;
pub use policy::*;
pub use schedule::*;
