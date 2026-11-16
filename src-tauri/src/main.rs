//! Desktop shell: runs the Recipes server in-process, shows it in a window on the loopback
//! listener, and keeps serving the network from the menu bar when the window is closed.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use recipes_core::server::{self, Config, Listener, Shell};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

struct TauriShell(AppHandle);

impl Shell for TauriShell {
    fn reveal(&self, abs: &Path) {
        let _ = self.0.opener().reveal_item_in_dir(abs);
    }
    fn open_url(&self, url: &str) {
        let _ = self.0.opener().open_url(url, None::<&str>);
    }
    fn choose_folder(&self) -> Option<PathBuf> {
        self.0.dialog().file().set_title("Choose your recipes folder").blocking_pick_folder()?.into_path().ok()
    }
    fn print(&self) {
        if let Some(w) = self.0.get_webview_window("main") {
            let w2 = w.clone();
            let _ = w.run_on_main_thread(move || {
                let _ = w2.print();
            });
        }
    }
    fn autostart(&self, set: Option<bool>) -> bool {
        let m = self.0.autolaunch();
        match set {
            Some(true) => {
                let _ = m.enable();
            }
            Some(false) => {
                let _ = m.disable();
            }
            None => {}
        }
        m.is_enabled().unwrap_or(false)
    }
    fn update_check(&self) -> Result<Option<(String, String)>, String> {
        let app = self.0.clone();
        tauri::async_runtime::block_on(async move {
            let found = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| format!("could not check for updates: {e}"))?;
            Ok(found.map(|u| (u.version, u.body.unwrap_or_default())))
        })
    }
    fn update_install(&self) -> Result<(), String> {
        // the bundle is replaced in place, which fails inside a disk image or a quarantined copy
        let exe = std::env::current_exe().unwrap_or_default().to_string_lossy().into_owned();
        if exe.starts_with("/Volumes/") || exe.contains("/AppTranslocation/") {
            return Err("Move Recipes to the Applications folder, open it from there, then update.".into());
        }
        let app = self.0.clone();
        tauri::async_runtime::block_on(async move {
            let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?.ok_or("Recipes is already up to date.")?;
            update.download_and_install(|_, _| {}, || {}).await.map_err(|e| format!("the update could not be installed: {e}"))
        })?;
        self.restart();
        Ok(())
    }
    fn restart(&self) {
        let app = self.0.clone();
        // answer the request first, then restart
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            app.restart();
        });
    }
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir()?;
            let vault = app.path().home_dir()?.join("Recipes");
            let shell: Arc<dyn Shell> = Arc::new(TauriShell(handle.clone()));
            // init spawns the file watcher, so it must run inside the async runtime
            let state = tauri::async_runtime::block_on(async move {
                server::init(Config { vault, data_dir, shell: Some(shell), pdf_text: recipes_core::pdf_text() })
            })?;

            let std_listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
            std_listener.set_nonblocking(true)?;
            let port = std_listener.local_addr()?.port();
            state.loopback_port.store(port, std::sync::atomic::Ordering::Relaxed);
            let st = state.clone();
            tauri::async_runtime::spawn(async move {
                let app = recipes_core::start_network(st).await;
                let l = tokio::net::TcpListener::from_std(std_listener).expect("loopback listener");
                if let Err(e) = server::serve(app, l, Listener::Loopback).await {
                    tracing::error!("server stopped: {e}");
                }
            });

            let url = format!("http://127.0.0.1:{port}/auth?k={}", state.launch_token);
            let opener = handle.clone();
            let opener2 = handle.clone();
            let downloads = app.path().download_dir().unwrap_or_else(|_| std::env::temp_dir());
            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse()?));
            #[cfg(target_os = "macos")]
            let builder = builder.title_bar_style(tauri::TitleBarStyle::Transparent);
            builder
                .title("") // no name in the title bar: the menu bar and Dock already say Recipes
                // the title bar takes the folder column's colour (`--sunken` #f3f1ea in app.css)
                .background_color(tauri::window::Color(0xf3, 0xf1, 0xea, 0xff))
                .inner_size(1240.0, 820.0)
                .min_inner_size(380.0, 480.0)
                .disable_drag_drop_handler() // let the page handle HTML5 drag & drop and file drops
                .on_navigation(move |u| {
                    let local = u.host_str() == Some("127.0.0.1");
                    if !local && (u.scheme() == "http" || u.scheme() == "https") {
                        let _ = opener.opener().open_url(u.as_str(), None::<&str>);
                    }
                    local
                })
                .on_new_window(move |u, _| {
                    let _ = opener2.opener().open_url(u.as_str(), None::<&str>);
                    NewWindowResponse::Deny
                })
                .on_download(move |_, ev| {
                    if let DownloadEvent::Requested { destination, .. } = ev {
                        let name = destination.file_name().map(|n| n.to_owned()).unwrap_or_else(|| "download".into());
                        let mut target = downloads.join(&name);
                        let mut n = 2;
                        while target.exists() {
                            let p = Path::new(&name);
                            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                            let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                            target = downloads.join(format!("{stem} ({n}){ext}"));
                            n += 1;
                        }
                        *destination = target;
                    }
                    true
                })
                .build()?;

            // menu-bar item: sharing keeps running while the window is closed
            let open = MenuItem::with_id(app, "open", "Open Recipes", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Recipes", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
            TrayIconBuilder::with_id("tray")
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .icon_as_template(true)
                .tooltip("Recipes")
                .menu(&menu)
                .on_menu_event(|app, e| match e.id().as_ref() {
                    "open" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(state);
            Ok(())
        })
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                // keep serving phones/tablets; quit from the menu-bar item
                api.prevent_close();
                let _ = w.hide();
                #[cfg(target_os = "macos")]
                let _ = w.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Recipes");

    app.run(|app, e| {
        if let RunEvent::Reopen { .. } = e {
            show(app);
        }
    });
}
