mod commands;
pub mod claude_hooks;
pub mod claude_proxy;
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
            
            // Set WebView2 background transparent BEFORE showing the window.
            // tauri.conf.json has visible:false so the window starts hidden.
            // set_background_color targets the OS window layer, NOT WebView2 itself.
            // We must call put_DefaultBackgroundColor via the WebView2 COM interface
            // directly so that CSS border-radius corners show the desktop, not grey.
            if let Some(window) = app.get_webview_window("widget") {
                #[cfg(target_os = "windows")]
                {
                    use webview2_com::Microsoft::Web::WebView2::Win32::{
                        ICoreWebView2Controller2, COREWEBVIEW2_COLOR,
                    };
                    use windows::core::Interface;
                    
                    // 1. Force the Win32 window to be fully transparent via window_vibrancy.
                    // Instead of brittle manual DWM edits, using clear_blur internally configures
                    // the exact layered window + composition attributes needed for a transparent background.
                    #[allow(unused_variables)]
                    let _ = window_vibrancy::apply_blur(&window, Some((0, 0, 0, 0)));
                    
                    // Or set clear_blur directly if apply_blur creates an effect:
                    let _ = window_vibrancy::clear_blur(&window);

                    // 2. Set WebView2 background transparent via COM
                    let _ = window.with_webview(|wv| {
                        unsafe {
                            if let Ok(ctrl) = wv.controller().cast::<ICoreWebView2Controller2>() {
                                ctrl.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR { A: 0, R: 0, G: 0, B: 0 }).ok();
                                eprintln!("[ding] WebView2 background set to transparent");
                            }
                        }
                    });
                }
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
            commands::submit_action,
            commands::kill_instance,
            commands::resize_widget,
            commands::quit_app,
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
