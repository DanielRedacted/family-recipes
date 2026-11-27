//! Desktop shell: runs the Recipes server in-process, shows it in a window on the loopback
//! listener, and keeps serving the network from the menu bar when the window is closed.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod pdf;

use recipes_core::server::{self, Config, Listener, Shell, Updates};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

struct TauriShell(AppHandle, Mutex<Option<Ready>>);

/// A newer release fetched in the background (Settings → App → Updates).
struct Ready {
    update: Update,
    /// The download, or None once it is installed and only a restart is left.
    bytes: Option<Vec<u8>>,
}

/// Written before restarting into an update while the window is closed, so the new version starts
/// in the menu bar too.
const RESTART_HIDDEN: &str = "restart-hidden";

/// The bundle is replaced in place, which fails inside a disk image or a quarantined copy.
fn installable() -> Result<(), String> {
    let exe = std::env::current_exe().unwrap_or_default().to_string_lossy().into_owned();
    if exe.starts_with("/Volumes/") || exe.contains("/AppTranslocation/") {
        return Err("Move Recipes to the Applications folder, open it from there, then update.".into());
    }
    Ok(())
}

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
    fn update_check(&self, mode: Updates) -> Result<Option<(String, String, bool)>, String> {
        let mut ready = self.1.lock().unwrap(); // one check (and download) at a time
        let app = self.0.clone();
        tauri::async_runtime::block_on(async move {
            let found = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| format!("could not check for updates: {e}"))?;
            let Some(u) = found else { return Ok(None) };
            if mode != Updates::Notify && installable().is_ok() {
                if !ready.as_ref().is_some_and(|r| r.update.version == u.version) {
                    let bytes = u.download(|_, _| {}, || {}).await.map_err(|e| format!("the update could not be downloaded: {e}"))?;
                    *ready = Some(Ready { update: u.clone(), bytes: Some(bytes) });
                }
                if mode == Updates::Install {
                    let r = ready.as_mut().unwrap();
                    if let Some(bytes) = r.bytes.take() {
                        if let Err(e) = r.update.install(bytes) {
                            *ready = None;
                            return Err(format!("the update could not be installed: {e}"));
                        }
                    }
                }
            }
            let is_ready = ready.as_ref().is_some_and(|r| r.update.version == u.version);
            Ok(Some((u.version, u.body.unwrap_or_default(), is_ready)))
        })
    }
    fn update_install(&self, quiet: bool) -> Result<bool, String> {
        installable()?;
        let visible = self.0.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        if quiet && visible {
            return Ok(false);
        }
        let mut ready = self.1.lock().unwrap();
        let app = self.0.clone();
        tauri::async_runtime::block_on(async move {
            let fail = |e: tauri_plugin_updater::Error| format!("the update could not be installed: {e}");
            match ready.take() {
                Some(Ready { bytes: None, .. }) => Ok(()), // installed in the background already
                Some(Ready { update, bytes: Some(bytes) }) => update.install(bytes).map_err(fail),
                None => {
                    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?.ok_or("Recipes is already up to date.")?;
                    update.download_and_install(|_, _| {}, || {}).await.map_err(fail)
                }
            }
        })?;
        if !visible {
            if let Ok(dir) = self.0.path().app_data_dir() {
                let _ = std::fs::write(dir.join(RESTART_HIDDEN), "");
            }
        }
        self.restart();
        Ok(true)
    }
    fn downloads(&self) -> Option<PathBuf> {
        self.0.path().download_dir().ok()
    }
    #[cfg(target_os = "macos")]
    fn save_pdf(&self, to: &Path) -> Result<(), String> {
        pdf::save(&self.0, to)
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

/// Started by "Open Recipes at login": serve phones and tablets from the menu bar, no window.
const HIDDEN: &str = "--hidden";

/// The release check (`scripts/release.mjs`): `RECIPES_SMOKE=<dir>` runs the built app on `<dir>/vault`
/// and `<dir>/data` with sharing off, no single-instance lock and no login item, so the installed app,
/// its settings and its login item are left alone. It builds the sharing certificate, then exits 0 once
/// the window shows the folders with no script errors, or 1. The window is shown (unfocused): WebKit
/// all but stops timers in a hidden one.
const SMOKE: &str = "RECIPES_SMOKE";
const SMOKE_SCRIPT: &str = r#"
  const errors = [];
  addEventListener('error', (e) => errors.push(String(e.message)));
  addEventListener('unhandledrejection', (e) => errors.push(String(e.reason)));
  const poll = setInterval(() => {
    if (!document.querySelector('a.folder')) return; // the app gives up after 45 s
    clearInterval(poll);
    location.href = '/__smoke?' + new URLSearchParams({ ok: errors.length ? '0' : '1', errors: errors.join('\n') });
  }, 250);
"#;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let smoke = std::env::var_os(SMOKE).map(PathBuf::from);
    let mut builder = tauri::Builder::default();
    if smoke.is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|a| a == HIDDEN) {
                show(app);
            }
        }));
    }
    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().arg(HIDDEN).build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let handle = app.handle().clone();
            let data_dir = match &smoke { Some(d) => d.join("data"), None => app.path().app_data_dir()? };
            let hidden = smoke.is_none() && (std::env::args().any(|a| a == HIDDEN) || std::fs::remove_file(data_dir.join(RESTART_HIDDEN)).is_ok());
            // a login item written by an older version starts without the flag: write it again
            // (not from the release check, which would point it at a build about to be deleted)
            if smoke.is_none() && !hidden && app.autolaunch().is_enabled().unwrap_or(false) {
                let _ = app.autolaunch().enable();
            }
            let vault = match &smoke { Some(d) => d.join("vault"), None => app.path().home_dir()?.join("Recipes") };
            let shell: Arc<dyn Shell> = Arc::new(TauriShell(handle.clone(), Mutex::new(None)));
            let dir = data_dir.clone();
            // init spawns the file watcher, so it must run inside the async runtime
            let state = tauri::async_runtime::block_on(async move {
                server::init(Config { vault, data_dir, shell: Some(shell), pdf_text: recipes_core::pdf_text() })
            })?;
            if smoke.is_some() {
                {
                    let mut s = state.settings.write().unwrap();
                    s.lan = false; // never advertise a test copy to the family's devices
                    s.setup_done = true;
                }
                tauri::async_runtime::block_on(recipes_core::lan::tls_config(&dir, &recipes_core::lan::lan_ips()))?;
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(45));
                    eprintln!("smoke: the window did not show the recipes within 45 s");
                    h.exit(1);
                });
            }

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
            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse()?));
            if smoke.is_some() {
                builder = builder.initialization_script(SMOKE_SCRIPT);
            }
            let (smoking, reporter) = (smoke.is_some(), handle.clone());
            #[cfg(target_os = "macos")]
            let builder = builder.title_bar_style(tauri::TitleBarStyle::Transparent);
            builder
                .title("") // no name in the title bar: the menu bar and Dock already say Recipes
                // the title bar takes the folder column's colour (`--sunken` #f1eef8 in app.css)
                .background_color(tauri::window::Color(0xf1, 0xee, 0xf8, 0xff))
                .inner_size(1240.0, 820.0)
                .min_inner_size(380.0, 480.0)
                .disable_drag_drop_handler() // let the page handle HTML5 drag & drop and file drops
                .visible(!hidden) // still loads (and signs in as the desktop app); `show` reveals it
                .focused(!smoking)
                .on_navigation(move |u| {
                    let local = u.host_str() == Some("127.0.0.1");
                    if smoking && local && u.path() == "/__smoke" {
                        let q: std::collections::HashMap<_, _> = u.query_pairs().collect();
                        let ok = q.get("ok").is_some_and(|v| v == "1");
                        if ok {
                            println!("smoke: ok");
                        } else {
                            eprintln!("smoke: the window did not show the recipes\n{}", q.get("errors").map(|e| e.as_ref()).unwrap_or(""));
                        }
                        reporter.exit(if ok { 0 } else { 1 });
                        return false;
                    }
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
                        let name = destination.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "download".into());
                        *destination = server::unique_file(&downloads, &name);
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
            #[cfg(target_os = "macos")]
            if hidden {
                let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory); // no Dock icon, as after closing
            }
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
