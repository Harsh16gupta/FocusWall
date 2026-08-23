//! FocusWall Daemon (`focuswalld`)
//!
//! Enforces DNS and firewall website blocking policies.

use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("FocusWall daemon starting...");
}
