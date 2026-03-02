mod bootstrap;
mod cli;
mod db;
mod import;
mod nav;
mod project;
mod shell;
#[allow(dead_code)]
mod style;

#[cfg(feature = "ai")]
mod ai;

#[cfg(feature = "tui")]
mod tui;

fn main() -> anyhow::Result<()> {
    cli::run()
}
