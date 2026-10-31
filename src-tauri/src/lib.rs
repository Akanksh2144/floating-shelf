mod export_cache;
mod island;
mod outbound_drag;
mod paths;
mod persist;
mod staging;
mod state;
mod window_ext;

use island::{reveal_island, set_island_bounds, set_island_expanded};
use persist::{init_storage, restore_app_state, save_app_state};
use staging::{persist_shelf_meta, stage_local_file};
use state::{AppState, Shelf};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WebviewWindowBuilder, WindowEvent,
};
use window_ext::apply_shelf_window_styles;

#[tauri::command]
fn get_active_shelf(state: State<'_, AppState>) -> Result<Option<Shelf>, String> {
    let active_id = state
        .active_shelf_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let Some(id) = active_id else {
        return Ok(None);
    };
    let shelves = state.shelves.lock().map_err(|e| e.to_string())?;
    Ok(shelves.get(&id).cloned())
}

#[tauri::command]
fn create_shelf(state: State<'_, AppState>, title: Option<String>) -> Result<Shelf, String> {
    let root = state
        .data_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "data root not initialized".to_string())?;

    let shelf = Shelf::new(title.unwrap_or_else(|| "Shelf".into()));
    persist_shelf_meta(&root, &shelf)?;

    {
        let mut shelves = state.shelves.lock().map_err(|e| e.to_string())?;
        shelves.insert(shelf.id.clone(), shelf.clone());
    }
    {
        let mut active = state.active_shelf_id.lock().map_err(|e| e.to_string())?;
        *active = Some(shelf.id.clone());
    }
    save_app_state(&root, &state)?;
    Ok(shelf)
}

#[tauri::command]
async fn stage_paths(
    app: AppHandle,
    state: State<'_, AppState>,
    shelf_id: String,
    paths: Vec<String>,
) -> Result<Shelf, String> {
    let root = state
        .data_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "data root not initialized".to_string())?;

    for path_str in paths {
        let path = std::path::PathBuf::from(&path_str);
        let mut shelf = {
            let shelves = state.shelves.lock().map_err(|e| e.to_string())?;
            shelves
                .get(&shelf_id)
                .cloned()
                .ok_or_else(|| "shelf not found".to_string())?
        };

        match stage_local_file(&app, &root, &mut shelf, &path).await {
            Ok(_) => {
                let mut shelves = state.shelves.lock().map_err(|e| e.to_string())?;
                shelves.insert(shelf_id.clone(), shelf);
            }
            Err(e) => {
                let mut shelves = state.shelves.lock().map_err(|e| e.to_string())?;
                shelves.insert(shelf_id.clone(), shelf);
                eprintln!("stage failed for {}: {e}", path.display());
            }
        }
    }

    save_app_state(&root, &state)?;
    let shelves = state.shelves.lock().map_err(|e| e.to_string())?;
    shelves
        .get(&shelf_id)
        .cloned()
        .ok_or_else(|| "shelf not found".to_string())
}

#[tauri::command]
fn remove_item(
    state: State<'_, AppState>,
    shelf_id: String,
    item_id: String,
) -> Result<Shelf, String> {
    let root = state
        .data_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "data root not initialized".to_string())?;

    let mut shelves = state.shelves.lock().map_err(|e| e.to_string())?;
    let shelf = shelves
        .get_mut(&shelf_id)
        .ok_or_else(|| "shelf not found".to_string())?;

    // Remove item record only — export-cache copies are retained by policy.
    shelf.items.retain(|i| i.id != item_id);
    shelf.updated_at = chrono::Utc::now();
    persist_shelf_meta(&root, shelf)?;
    let result = shelf.clone();
    drop(shelves);
    save_app_state(&root, &state)?;
    Ok(result)
}

#[tauri::command]
fn show_shelf_window(app: AppHandle) -> Result<(), String> {
    reveal_island(app)
}

fn ensure_active_shelf(state: &AppState, root: &std::path::PathBuf) -> Result<Shelf, String> {
    {
        let active = state.active_shelf_id.lock().map_err(|e| e.to_string())?;
        let shelves = state.shelves.lock().map_err(|e| e.to_string())?;
        if let Some(id) = active.as_ref() {
            if let Some(shelf) = shelves.get(id) {
                return Ok(shelf.clone());
            }
        }
    }

    let shelf = Shelf::new("Shelf");
    persist_shelf_meta(root, &shelf)?;
    {
        let mut shelves = state.shelves.lock().map_err(|e| e.to_string())?;
        shelves.insert(shelf.id.clone(), shelf.clone());
    }
    {
        let mut active = state.active_shelf_id.lock().map_err(|e| e.to_string())?;
        *active = Some(shelf.id.clone());
    }
    save_app_state(root, state)?;
    Ok(shelf)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_active_shelf,
            create_shelf,
            stage_paths,
            remove_item,
            show_shelf_window,
            set_island_expanded,
            set_island_bounds,
            reveal_island,
            outbound_drag::start_outbound_drag,
        ])
        .setup(|app| {
            let root = paths::data_root(app.handle())?;
            init_storage(&root)?;

            {
                let state = app.state::<AppState>();
                *state.data_root.lock().unwrap() = Some(root.clone());
                let _ = restore_app_state(&root, &state);
                let _ = ensure_active_shelf(&state, &root);
            }

            if let Some(win) = app.get_webview_window("shelf") {
                let _ = apply_shelf_window_styles(&win);
                let _ = island::dock_right_edge(
                    &win,
                    island::COLLAPSED_W,
                    island::COLLAPSED_H,
                );
                let _ = win.show();
            } else {
                let win = WebviewWindowBuilder::new(
                    app,
                    "shelf",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("Modillion")
                .inner_size(island::COLLAPSED_W, island::COLLAPSED_H)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .transparent(false)
                .build()?;
                let _ = apply_shelf_window_styles(&win);
                let _ = island::dock_right_edge(
                    &win,
                    island::COLLAPSED_W,
                    island::COLLAPSED_H,
                );
            }

            let reveal_i =
                MenuItem::with_id(app, "reveal", "Reveal Island", true, None::<&str>)?;
            let settings_i =
                MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&reveal_i, &settings_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Modillion")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "reveal" => {
                        let _ = reveal_island(app.clone());
                    }
                    "settings" => {
                        // Settings UI lands with licensing; tray entry is wired now.
                        let _ = reveal_island(app.clone());
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let _ = reveal_island(app.clone());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "shelf" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    // Collapse to pill instead of destroying — utility stays alive.
                    api.prevent_close();
                    let app = window.app_handle().clone();
                    let _ = set_island_expanded(app, false);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
