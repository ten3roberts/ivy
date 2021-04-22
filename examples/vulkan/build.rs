use std::process::Command;

fn main() {
    let status = Command::new("make")
        .status()
        .expect("Failed to run `make` command");

    if !status.success() {
        panic!("Failed to run Makefile");
    }
}
