use gtk::gio;
use gtk::prelude::*;
use pin_native::{
    board::{valid_date, Board, COLORS},
    paths::{config_root, data_file},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;

thread_local! { static APPEARANCE: RefCell<Option<gio::Settings>> = const { RefCell::new(None) }; }

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    width: f64,
    height: f64,
    x: Option<i32>,
    y: Option<i32>,
    pinned: Option<bool>,
    folded: HashSet<String>,
}
struct Shared {
    notes: bool,
    file: PathBuf,
    config: PathBuf,
    settings: Mutex<Settings>,
    dark: AtomicBool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Document {
    text: String,
    board: Option<Board>,
}
#[derive(Serialize)]
struct Boot {
    notes: bool,
    disk: Option<String>,
    canonical: String,
    document: Document,
    pinned: bool,
    folded: Vec<String>,
    colors: Vec<(String, String)>,
    dark: bool,
}
#[derive(Serialize)]
struct Saved {
    saved: bool,
    disk: Option<String>,
    content: String,
}

fn persist(shared: &Shared) {
    let settings = shared
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let result = serde_json::to_string_pretty(&settings)
        .map_err(std::io::Error::other)
        .and_then(|text| {
            storage::read(&shared.config)
                .and_then(|expected| storage::save(&shared.config, &text, &expected))
        });
    if let Err(error) = result {
        eprintln!("Window settings: {error}");
    }
}

#[tauri::command]
fn get_state(shared: tauri::State<'_, Shared>) -> Result<Boot, String> {
    let disk = storage::read(&shared.file).map_err(|e| e.to_string())?;
    let text = disk.clone().unwrap_or_default();
    let board = (!shared.notes).then(|| Board::parse(&text));
    let canonical = board.as_ref().map_or_else(|| text.clone(), Board::markdown);
    let settings = shared.settings.lock().unwrap_or_else(|e| e.into_inner());
    Ok(Boot {
        notes: shared.notes,
        disk,
        canonical,
        document: Document {
            text: if shared.notes { text } else { String::new() },
            board,
        },
        pinned: settings.pinned.unwrap_or(true),
        folded: settings.folded.iter().cloned().collect(),
        colors: COLORS
            .iter()
            .map(|(id, hex)| (id.to_string(), hex.to_string()))
            .collect(),
        dark: shared.dark.load(Ordering::Relaxed),
    })
}
#[tauri::command]
fn read_disk(shared: tauri::State<'_, Shared>) -> Result<Option<String>, String> {
    storage::read(&shared.file).map_err(|e| e.to_string())
}
#[tauri::command]
fn write_document(
    document: Document,
    expected: Option<String>,
    shared: tauri::State<'_, Shared>,
) -> Result<Saved, String> {
    let content = if shared.notes {
        if document.board.is_some() {
            return Err("Invalid notes document".into());
        }
        document.text
    } else {
        let board = document.board.ok_or("Missing task list")?;
        let mut ids = HashSet::new();
        let mut depth = 0;
        for (i, t) in board.tasks.iter().enumerate() {
            if !ids.insert(t.id)
                || (i == 0 && t.depth != 0)
                || t.depth > depth + 1
                || t.text.contains(['\r', '\n'])
                || t.date.as_deref().is_some_and(|s| !valid_date(s))
            {
                return Err("Invalid task data".into());
            }
            if let Some(c) = &t.color {
                if !COLORS.iter().any(|(id, _)| id == c)
                    && !(c.len() == 7
                        && c.starts_with('#')
                        && c[1..].chars().all(|c| c.is_ascii_hexdigit()))
                {
                    return Err("Invalid task color".into());
                }
            }
            depth = t.depth;
        }
        board.markdown()
    };
    let saved = storage::save(&shared.file, &content, &expected).map_err(|e| e.to_string())?;
    let disk = if saved {
        Some(content.clone())
    } else {
        storage::read(&shared.file).map_err(|e| e.to_string())?
    };
    Ok(Saved {
        saved,
        disk,
        content,
    })
}
#[tauri::command]
fn pin_window(
    on: bool,
    window: WebviewWindow,
    shared: tauri::State<'_, Shared>,
) -> Result<(), String> {
    window.set_always_on_top(on).map_err(|e| e.to_string())?;
    shared
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pinned = Some(on);
    persist(&shared);
    Ok(())
}
#[tauri::command]
fn save_view(folded: Vec<String>, shared: tauri::State<'_, Shared>) {
    shared
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .folded = folded.into_iter().collect();
    persist(&shared);
}
#[tauri::command]
fn show_ready(window: WebviewWindow) -> Result<(), String> {
    window
        .show()
        .and_then(|_| window.set_focus())
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn read_clipboard(app: tauri::AppHandle) -> Result<String, String> {
    app.clipboard().read_text().map_err(|e| e.to_string())
}
#[tauri::command]
fn write_clipboard(text: String, app: tauri::AppHandle) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

fn watch_theme(app: &tauri::AppHandle) {
    let Some(schema) = gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup("org.gnome.desktop.interface", true))
        .filter(|s| s.has_key("color-scheme"))
    else {
        return;
    };
    let settings = gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None);
    let update = |s: &gio::Settings, app: &tauri::AppHandle| {
        let dark = s.string("color-scheme") == "prefer-dark";
        if let Some(gtk) = gtk::Settings::default() {
            gtk.set_gtk_application_prefer_dark_theme(dark);
        }
        app.state::<Shared>().dark.store(dark, Ordering::Relaxed);
        if let Some(window) = app.get_webview_window("main") {
            resize_background(&window, dark);
        }
        let _ = app.emit("desktop-theme", dark);
    };
    update(&settings, app);
    let handle = app.clone();
    settings.connect_changed(Some("color-scheme"), move |s, _| update(s, &handle));
    APPEARANCE.with(|slot| *slot.borrow_mut() = Some(settings));
}

#[allow(deprecated)] // GDK's X11 resize background has no GTK 3 replacement.
fn resize_background(window: &WebviewWindow, dark: bool) {
    use gtk::glib::translate::ToGlibPtr;
    let rgb = if dark { [25., 25., 28.] } else { [252.; 3] };
    if let Err(error) = window.set_background_color(Some(tauri::utils::config::Color(
        rgb[0] as u8,
        rgb[1] as u8,
        rgb[2] as u8,
        255,
    ))) {
        eprintln!("Window background: {error}");
    }
    if let Ok(window) = window.gtk_window() {
        if let Some(surface) = window.window() {
            let color = gtk::gdk::RGBA::new(rgb[0] / 255., rgb[1] / 255., rgb[2] / 255., 1.);
            // The safe GTK 3 wrapper omits this deprecated GDK window API.
            unsafe {
                gtk::gdk::ffi::gdk_window_set_background_rgba(
                    surface.to_glib_none().0,
                    color.to_glib_none().0,
                );
            }
        }
    }
}

pub fn run() {
    let notes = std::env::args().any(|s| s == "--notes")
        || std::env::args()
            .next()
            .is_some_and(|s| s.ends_with("sticky-notes"));
    gtk::glib::set_prgname(Some(if notes { "sticky-notes" } else { "pin" }));
    gtk::glib::set_application_name(if notes { "Sticky Notes" } else { "Pin" });
    let config = config_root()
        .join(if notes { "StickyNotes" } else { "Pin" })
        .join("native.json");
    let settings = std::fs::read_to_string(&config)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default();
    let file = data_file(notes);
    let shared = Shared {
        notes,
        file,
        config,
        settings: Mutex::new(settings.clone()),
        dark: AtomicBool::new(false),
    };
    let mut context = tauri::generate_context!();
    if notes {
        context.config_mut().identifier = "io.github.masihmoafi.StickyNotes".into();
        context.config_mut().product_name = Some("Sticky Notes".into());
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            get_state,
            read_disk,
            write_document,
            pin_window,
            save_view,
            show_ready,
            read_clipboard,
            write_clipboard
        ])
        .setup(move |app| {
            watch_theme(app.handle());
            let width = if settings.width >= 280. {
                settings.width
            } else {
                460.
            };
            let height = if settings.height >= 180. {
                settings.height
            } else if notes {
                380.
            } else {
                720.
            };
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title(if notes { "Sticky Notes" } else { "Pin" })
                    .inner_size(width, height)
                    .min_inner_size(280., 180.)
                    .decorations(false)
                    .transparent(true)
                    .resizable(true)
                    .always_on_top(settings.pinned.unwrap_or(true))
                    .visible(false)
                    .build()?;
            resize_background(&window, app.state::<Shared>().dark.load(Ordering::Relaxed));
            let icon = tauri::image::Image::from_bytes(if notes {
                include_bytes!("../../logo/notes-256.png")
            } else {
                include_bytes!("../../logo/pin-256.png")
            })?;
            window.set_icon(icon)?;
            if let Some(monitor) = window.primary_monitor()? {
                let scale = monitor.scale_factor();
                let origin = monitor.position();
                let size = monitor.size();
                let x = settings
                    .x
                    .unwrap_or(origin.x + size.width as i32 - (width * scale) as i32 - 20)
                    .clamp(
                        origin.x,
                        origin.x + (size.width as i32 - (width * scale) as i32).max(0),
                    );
                let y = settings.y.unwrap_or(origin.y + 60).clamp(
                    origin.y,
                    origin.y + (size.height as i32 - (height * scale) as i32).max(0),
                );
                window.set_position(tauri::PhysicalPosition::new(x, y))?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let shared = window.state::<Shared>();
            match event {
                tauri::WindowEvent::Moved(p) => {
                    let mut s = shared.settings.lock().unwrap_or_else(|e| e.into_inner());
                    s.x = Some(p.x);
                    s.y = Some(p.y);
                }
                tauri::WindowEvent::Resized(size) => {
                    if !window.is_maximized().unwrap_or(false) {
                        let scale = window.scale_factor().unwrap_or(1.);
                        let mut s = shared.settings.lock().unwrap_or_else(|e| e.into_inner());
                        s.width = f64::from(size.width) / scale;
                        s.height = f64::from(size.height) / scale;
                    }
                }
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
                    persist(&shared)
                }
                _ => (),
            }
        })
        .run(context)
        .expect("Could not start Pin");
}
