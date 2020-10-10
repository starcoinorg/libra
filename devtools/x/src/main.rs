// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use chrono::Local;
use env_logger::{self, fmt::Color};
use log::Level;
use std::io::Write;
use structopt::StructOpt;

type Result<T> = anyhow::Result<T>;

#[derive(Debug, StructOpt)]
struct Args {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "bench")]
    /// Run `cargo bench`
    Bench(x::bench::Args),
    #[structopt(name = "check")]
    /// Run `cargo check`
    Check(x::check::Args),
    #[structopt(name = "clippy")]
    /// Run `cargo clippy`
    Clippy(x::clippy::Args),
    #[structopt(name = "fix")]
    /// Run `cargo fix`
    Fix(x::fix::Args),
    #[structopt(name = "fmt")]
    /// Run `cargo fmt`
    Fmt(x::fmt::Args),
    #[structopt(name = "test")]
    /// Run tests
    Test(x::test::Args),
    #[structopt(name = "tools")]
    /// Run tests
    Tools(x::tools::Args),
    #[structopt(name = "lint")]
    /// Run lints
    Lint(x::lint::Args),
    #[structopt(name = "generate-summaries")]
    /// Generate build summaries for important subsets
    GenerateSummaries(x::generate_summaries::Args),
    #[structopt(name = "diff-summary")]
    /// Diff build summaries for important subsets
    DiffSummary(x::diff_summary::Args),
}

fn main() -> Result<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let color = match record.level() {
                Level::Warn => Color::Yellow,
                Level::Error => Color::Red,
                _ => Color::Green,
            };

            let mut level_style = buf.style();
            level_style.set_color(color).set_bold(true);

            writeln!(
                buf,
                "{:>12} [{}] - {}",
                level_style.value(record.level()),
                Local::now().format("%T%.3f"),
                record.args()
            )
        })
        .init();

    let args = Args::from_args();
    let xctx = x::context::XContext::new()?;

    match args.cmd {
        Command::Tools(args) => x::tools::run(args, xctx),
        Command::Test(args) => test::run(args, xctx),
        Command::Check(args) => x::check::run(args, xctx),
        Command::Clippy(args) => x::clippy::run(args, xctx),
        Command::Fix(args) => x::fix::run(args, xctx),
        Command::Fmt(args) => x::fmt::run(args, xctx),
        Command::Bench(args) => x::bench::run(args, xctx),
        Command::Lint(args) => x::lint::run(args, xctx),
        Command::GenerateSummaries(args) => x::generate_summaries::run(args, xctx),
        Command::DiffSummary(args) => x::diff_summary::run(args, xctx),
    }
}
