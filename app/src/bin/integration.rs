use anyhow::Result;
use clap::Parser;
use rift_cli::WorkerCommand;
use rift_core::channel::{Channel, ChannelConfig, ChannelState};
use rift_core::AppId;

#[derive(Debug, Default, Parser, Clone)]
#[command(name = "rift-integration")]
#[clap(args_conflicts_with_subcommands = true)]
pub struct Args {
    #[command(subcommand)]
    command: Option<WorkerCommand>,
}

pub fn main() -> Result<()> {
    ChannelState::set(ChannelState::new(
        Channel::Integration,
        ChannelConfig {
            app_id: AppId::new(
                "dev",
                "rift",
                if cfg!(target_os = "macos") {
                    "Rift-Integration"
                } else {
                    "RiftIntegration"
                },
            ),
            logfile_name: "rift_integration.log".into(),
        },
    ));

    let args = Args::parse();

    if let Some(command) = &args.command {
        match command {
            #[cfg(unix)]
            WorkerCommand::TerminalServer(args) => {
                // If we were asked to run as a terminal server (as opposed to the main
                // GUI application), do so.  This must occur before init_logging, as the
                // terminal server sets up its own logger, and attempting to set a second
                // logger leads to a panic.
                rift::terminal::local_tty::server::run_terminal_server(args);
                return Ok(());
            }
            // This is a catch-all to handle the plugin host, which the integration test crate doesn't have a feature flag for.
            #[allow(unreachable_patterns)]
            other => panic!("Worker not supported in integration tests: {other:?}"),
        }
    }

    rift::run()
}
