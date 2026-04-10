mod commands;
pub mod claude_hooks;
mod instance;
mod events;
pub mod cli;
mod adapter;
pub mod ipc;
pub mod monitor;

use std::future::Future;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use instance::manager::InstanceManager;

pub type SharedManager = Arc<Mutex<InstanceManager>>;

fn spawn_background_future<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    std::thread::Builder::new()
        .name("ding-bg-runtime".to_string())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create background tokio runtime");

                runtime.block_on(future);
            }));

            if let Err(payload) = result {
                if let Some(message) = payload.downcast_ref::<&str>() {
                    eprintln!("background runtime thread panicked: {message}");
                } else if let Some(message) = payload.downcast_ref::<String>() {
                    eprintln!("background runtime thread panicked: {message}");
                } else {
                    eprintln!("background runtime thread panicked with non-string payload");
                }
            }
        })
        .expect("failed to spawn background runtime thread");
}

pub fn run() {
    let manager = Arc::new(Mutex::new(InstanceManager::new()));
    let ipc_manager = manager.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(manager)
        .setup(move |app| {
            let handle = app.handle().clone();
            let manager = ipc_manager.clone();
            
            // Force window visibility and center it
            if let Some(window) = app.get_webview_window("widget") {
                let _ = window.center();
                let _ = window.show();
            }

            spawn_background_future(async move {
                if let Err(e) = ipc::start_ipc_server_with_manager(manager, Some(handle)).await {
                    eprintln!("IPC Server Error: {}", e);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_instances,
            commands::create_claude_instance,
            commands::create_codex_instance,
            commands::send_decision,
            commands::kill_instance,
            commands::resize_widget,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn background_runtime_executes_async_work_without_ambient_tokio_runtime() {
        let (tx, rx) = mpsc::channel();

        super::spawn_background_future(async move {
            tx.send(42usize).unwrap();
        });

        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 42);
    }
}
