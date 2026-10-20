//! The app's sync modules, compiled here as they are in the app.

#[path = "../../../../src-tauri/src/sync/engine.rs"]
pub mod engine;
#[path = "../../../../src-tauri/src/sync/hub.rs"]
pub mod hub;
#[path = "../../../../src-tauri/src/sync/identity.rs"]
pub mod identity;
#[path = "../../../../src-tauri/src/sync/kinds.rs"]
pub mod kinds;
#[path = "../../../../src-tauri/src/sync/ledger.rs"]
pub mod ledger;
#[path = "../../../../src-tauri/src/sync/link.rs"]
pub mod link;
#[path = "../../../../src-tauri/src/sync/protocol.rs"]
pub mod protocol;
