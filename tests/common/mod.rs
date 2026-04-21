#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

pub fn run_rstube(dir: &Path, args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_rstube");
    Command::new(bin)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("Failed to execute rstube")
}
