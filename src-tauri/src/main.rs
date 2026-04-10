use clap::Parser;
use std::env;

fn main() {
    // Check if we are running as daemon.
    // When tauri runs, if there are no arguments (or just the executable name),
    // it functions as a daemon. Otherwise it's a CLI command.
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        // CLI mode
        let cli = ding_lib::cli::Cli::parse();
        if let Some(cmd) = cli.command {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                ding_lib::cli::commands::handle_command(cmd).await;
            });
        }
    } else {
        // Daemon mode
        println!("Starting daemon...");
        ding_lib::run();
    }
}
