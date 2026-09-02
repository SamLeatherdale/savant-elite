//! Library seams for the Savant Elite CLI.
//!
//! This crate is a structural split of the former monolithic binary. The active
//! `savant program` path uses the verified encoder and a single write. Play-mode
//! observation lives in [`monitor`].

pub mod cli;
pub mod config;
pub mod monitor;
pub mod platform;
pub mod protocol;
pub mod transport;

pub use cli::run;
