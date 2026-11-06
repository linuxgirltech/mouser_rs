//! mouserd — exposes libmouser over D-Bus.
//!
//! Skeleton: argument parsing and logging are wired up, service setup is not.

// TODO: delete once the `todo!()` bodies are filled in.
#![allow(unused_variables, dead_code)]

mod dbus;
mod names;
mod state;

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "mouserd", version, about = "libmouser D-Bus daemon")]
struct Args {
    /// Connect to the session bus instead of the system bus. For development:
    /// no root, no policy file, and `busctl --user` can see it.
    #[arg(long)]
    session: bool,

    /// Where the `.device` database lives. Defaults to $LIBMOUSER_DATA_DIR,
    /// else the compiled-in path.
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Probe only this node rather than enumerating. Repeatable.
    #[arg(long, value_name = "PATH")]
    device: Vec<PathBuf>,

    /// -v for debug, -vv for trace. `RUST_LOG` overrides this.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_tracing(args.verbose);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(args))
}

fn init_tracing(verbose: u8) {
    use tracing_subscriber::EnvFilter;

    let default = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("mouserd={default},libmouser={default}")));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

/// Bring the daemon up and run until shut down.
///
/// The order that matters:
///
/// 1. build the `libmouser::Context` from the data dir;
/// 2. probe devices — `args.device` if given, else `Context::probe_all`;
/// 3. build the connection with `zbus::connection::Builder::{system,session}`,
///    serving the `Manager` at [`names::MANAGER_PATH`] and every device's
///    object tree, then request [`names::BUS_NAME`] **last** — a client that
///    sees the name appear must find a fully populated tree;
/// 4. register the udev monitor's fd with `tokio::io::unix::AsyncFd` and
///    add/remove objects as devices come and go, emitting
///    `PropertiesChanged` for `Manager.Devices` each time;
/// 5. wait on SIGINT/SIGTERM via `tokio::signal`.
///
/// A device needs its whole subtree exported: Device, each Profile, and each
/// Resolution, Button and Led within each Profile. That is the loop worth
/// factoring out first.
async fn run(args: Args) -> anyhow::Result<()> {
    todo!()
}
