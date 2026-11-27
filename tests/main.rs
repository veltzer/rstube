// Everything here is test code; clippy.toml lets test code unwrap.
#![cfg(test)]

mod common;

#[path = "tests_mod/persistence.rs"]
mod persistence;
#[path = "tests_mod/version.rs"]
mod version;
