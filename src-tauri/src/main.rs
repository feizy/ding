use clap::Parser;
use std::env;

fn main() {
    // Check if we are running as daemon.
    // When tauri runs, if there are no arguments (or just the executable name),
    // it functions as a daemon. Otherwise it's a CLI command.
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        // CLI mode
        attach_parent_console_for_cli();
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

#[cfg(windows)]
fn attach_parent_console_for_cli() {
    use windows::core::w;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Console::{
        AttachConsole, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
        STD_OUTPUT_HANDLE,
    };

    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);

        if let Ok(handle) = CreateFileW(
            w!("CONIN$"),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        ) {
            if !handle.is_invalid() {
                let _ = SetStdHandle(STD_INPUT_HANDLE, handle);
            }
        }

        if let Ok(stdout) = open_console_output() {
            if !stdout.is_invalid() {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, stdout);
            }
        }

        if let Ok(stderr) = open_console_output() {
            if !stderr.is_invalid() {
                let _ = SetStdHandle(STD_ERROR_HANDLE, stderr);
            }
        }
    }
}

#[cfg(windows)]
unsafe fn open_console_output() -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
    use windows::core::w;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    CreateFileW(
        w!("CONOUT$"),
        FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        None,
    )
}

#[cfg(not(windows))]
fn attach_parent_console_for_cli() {}
