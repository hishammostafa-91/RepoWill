// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "repowill",
    version,
    about = "RepoWill — repository rules, gates and guardrails"
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
    println!("repowill {}", env!("CARGO_PKG_VERSION"));
}
