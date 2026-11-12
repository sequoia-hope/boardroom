//! Boardroom's HTTP server: the API over projects, the apps under one origin, and the
//! terminals. It calls the library crates; it holds no project logic of its own.

pub mod apps;
pub mod server;
pub mod terminals;
