use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetAncestor, GetForegroundWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, IsWindow, IsWindowVisible, GA_ROOTOWNER, GWL_EXSTYLE, GWL_STYLE,
    WS_CAPTION, WS_CHILD, WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW, WS_POPUP,
};

const SLOT_COUNT: usize = 64;
const TRACK_INTERVAL_MILLIS: u64 = 100;
const REFERENCE_WND_CHANGE_FRAME_COUNT: usize = 3;
const BAR_HEIGHT: i32 = 124;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    None,
    Folder,
    Exe,
    Script,
    Url,
    Text,
    Image,
    Doc,
    Cpl,
    OtherApp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Classic,
    Dark,
    Light,
}

fn default_theme() -> Theme {
    Theme::Classic
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSlot {
    pub index: usize,
    #[serde(rename = "dataType")]
    pub data_type: DataType,
    pub path: String,
    pub arg: String,
    pub comment: String,
    pub exist: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSettings {
    pub version: i32,
    #[serde(default = "default_theme")]
    pub theme: Theme,
    pub slots: Vec<LauncherSlot>,
}

#[derive(Debug, Default)]
struct TrackingState {
    thread_started: bool,
    lock_enabled: bool,
    put_in_tray: bool,
    reference_hwnd: isize,
    pending_hwnd: isize,
    pending_count: usize,
    last_visible: bool,
}

static TRACKING_STATE: OnceLock<Mutex<TrackingState>> = OnceLock::new();

fn tracking_state() -> &'static Mutex<TrackingState> {
    TRACKING_STATE.get_or_init(|| Mutex::new(TrackingState::default()))
}

fn empty_slot(index: usize) -> LauncherSlot {
    LauncherSlot {
        index,
        data_type: DataType::None,
        path: String::new(),
        arg: String::new(),
        comment: String::new(),
        exist: false,
    }
}

fn normalize_settings(mut settings: LauncherSettings) -> LauncherSettings {
    let mut slots = Vec::with_capacity(SLOT_COUNT);
    for index in 0..SLOT_COUNT {
        if let Some(slot) = settings.slots.get(index).cloned() {
            let exist = disk_path_exists(&slot.path, &slot.data_type);
            let normalized = LauncherSlot {
                index,
                data_type: slot.data_type,
                path: slot.path,
                arg: slot.arg,
                comment: slot.comment,
                exist,
            };
            slots.push(normalized);
        } else {
            slots.push(empty_slot(index));
        }
    }

    settings.version = 1;
    settings.slots = slots;
    settings
}

fn default_settings() -> LauncherSettings {
    let slots = (0..SLOT_COUNT).map(empty_slot).collect();
    LauncherSettings {
        version: 1,
        theme: Theme::Classic,
        slots,
    }
}

fn file_modified_time(path: &Path) -> SystemTime {
    path.metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .unwrap_or(UNIX_EPOCH)
}

fn settings_file_path(app: &AppHandle) -> Option<PathBuf> {
    let app_dir = app.path().app_data_dir().ok()?;
    let folder = app_dir.join("KActiveWindowLauncher");
    let _ = fs::create_dir_all(&folder);
    Some(folder.join("settings.json"))
}

fn backup_file_paths(app: &AppHandle) -> Vec<PathBuf> {
    let app_dir = match app.path().app_data_dir() {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    let folder = app_dir.join("KActiveWindowLauncher");
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut files = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().contains("settings_backup_"))
        })
        .collect::<Vec<_>>();

    files.sort_by(|lhs, rhs| file_modified_time(rhs).cmp(&file_modified_time(lhs)));
    files
}

fn write_settings(app: &AppHandle, settings: &LauncherSettings) -> Result<(), String> {
    let normalized = normalize_settings(settings.clone());
    let file_path = settings_file_path(app)
        .ok_or_else(|| "設定ファイルの保存先を決定できませんでした".to_string())?;
    let contents = serde_json::to_string_pretty(&normalized).map_err(|err| err.to_string())?;
    fs::write(&file_path, contents).map_err(|err| err.to_string())?;

    let backup_dir = file_path.parent().unwrap_or(Path::new("."));
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_millis();
    let backup_path = backup_dir.join(format!("settings_backup_{unix_ms}.json"));
    fs::copy(&file_path, &backup_path).map_err(|err| err.to_string())?;

    let mut backups = backup_file_paths(app);
    backups.push(backup_path);
    backups.sort_by(|lhs, rhs| file_modified_time(rhs).cmp(&file_modified_time(lhs)));
    while backups.len() > 5 {
        let old = backups.pop().expect("backups is not empty");
        let _ = fs::remove_file(old);
    }
    Ok(())
}

fn emit_settings_updated(app: &AppHandle) -> Result<(), String> {
    app.emit("launcher://settings-updated", serde_json::json!({}))
        .map_err(|err| err.to_string())
}

fn load_settings(app: &AppHandle) -> Result<LauncherSettings, String> {
    let main_path =
        settings_file_path(app).ok_or_else(|| "設定ファイルの場所が確認できません".to_string())?;
    if main_path.exists() {
        let bytes = fs::read(&main_path).map_err(|err| err.to_string())?;
        if !bytes.is_empty() {
            if let Ok(settings) = serde_json::from_slice::<LauncherSettings>(&bytes) {
                return Ok(normalize_settings(settings));
            }
        }
    }

    let mut backups = backup_file_paths(app);
    backups.sort_by(|lhs, rhs| file_modified_time(rhs).cmp(&file_modified_time(lhs)));
    for backup in backups {
        if let Ok(content) = fs::read(&backup) {
            if !content.is_empty() {
                if let Ok(settings) = serde_json::from_slice::<LauncherSettings>(&content) {
                    return Ok(normalize_settings(settings));
                }
            }
        }
    }
    Ok(default_settings())
}

fn detect_data_type(path: &str) -> DataType {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return DataType::None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let path_ref = Path::new(trimmed);

    if path_ref.is_dir() {
        return DataType::Folder;
    }
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www") {
        return DataType::Url;
    }

    match path_ref
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|value| value.to_ascii_lowercase())
    {
        Some(ext) if ext == "exe" => DataType::Exe,
        Some(ext) if ext == "vbs" || ext == "bat" => DataType::Script,
        Some(ext) if matches!(ext.as_str(), "bmp" | "jpg" | "jpe" | "jpeg" | "png" | "tga") => {
            DataType::Image
        }
        Some(ext) if ext == "txt" => DataType::Text,
        Some(ext) if matches!(ext.as_str(), "htm" | "html") => DataType::Url,
        Some(ext) if matches!(ext.as_str(), "ppt" | "xls" | "doc" | "pdf") => DataType::Doc,
        Some(ext) if ext == "cpl" => DataType::Cpl,
        _ => DataType::OtherApp,
    }
}

fn disk_path_exists(path: &str, data_type: &DataType) -> bool {
    let candidate = path.trim();
    if candidate.is_empty() {
        return false;
    }
    let target = Path::new(candidate);
    match data_type {
        DataType::Folder => target.is_dir(),
        DataType::Url => {
            candidate.contains("http://")
                || candidate.contains("https://")
                || candidate.starts_with("www")
        }
        _ => target.exists(),
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn is_valid_window(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe { IsWindow(hwnd as HWND) != 0 }
}

fn is_excluded_window(hwnd: isize) -> bool {
    if hwnd == 0 {
        return true;
    }

    unsafe {
        if IsWindow(hwnd as HWND) == 0 || IsWindowVisible(hwnd as HWND) == 0 {
            return true;
        }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd as HWND, &mut process_id);
        if process_id == GetCurrentProcessId() {
            return true;
        }

        let root_owner = GetAncestor(hwnd as HWND, GA_ROOTOWNER);
        if root_owner != std::ptr::null_mut() && root_owner != hwnd as HWND {
            return true;
        }

        let taskbar_class = to_wide("Shell_TrayWnd");
        let tray_class = to_wide("TrayNotifyWnd");
        let taskbar = FindWindowW(taskbar_class.as_ptr(), std::ptr::null());
        let tray = FindWindowExW(
            taskbar,
            std::ptr::null_mut(),
            tray_class.as_ptr(),
            std::ptr::null(),
        );
        if hwnd as HWND == taskbar || hwnd as HWND == tray {
            return true;
        }

        let style_ex = GetWindowLongW(hwnd as HWND, GWL_EXSTYLE) as u32;
        if style_ex & WS_EX_TOOLWINDOW != 0 || style_ex & WS_EX_DLGMODALFRAME != 0 {
            return true;
        }

        let style = GetWindowLongW(hwnd as HWND, GWL_STYLE) as u32;
        if style & WS_CHILD != 0 || style & WS_POPUP != 0 || style & WS_CAPTION == 0 {
            return true;
        }
    }

    false
}

fn get_window_rect(hwnd: isize) -> Option<RECT> {
    if hwnd == 0 {
        return None;
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(hwnd as HWND, &mut rect) != 0 };
    if !ok {
        return None;
    }
    Some(rect)
}

fn emit_reference_changed(
    window: &WebviewWindow,
    reference_hwnd: isize,
    lock_enabled: bool,
) -> Result<(), String> {
    if reference_hwnd == 0 {
        window
            .emit(
                "launcher://reference-window-changed",
                serde_json::json!({ "hwnd": null, "locked": lock_enabled }),
            )
            .map_err(|err| err.to_string())
    } else {
        window
            .emit(
                "launcher://reference-window-changed",
                serde_json::json!({ "hwnd": format!("{reference_hwnd:#x}"), "locked": lock_enabled }),
            )
            .map_err(|err| err.to_string())
    }
}

fn set_put_in_tray(value: bool) -> Result<(), String> {
    let mut state = tracking_state()
        .lock()
        .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
    state.put_in_tray = value;
    Ok(())
}

fn update_tracking(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main ウインドウが見つかりません".to_string())?;

    let current_hwnd = unsafe { GetForegroundWindow() } as isize;
    let candidate_hwnd = if is_excluded_window(current_hwnd) {
        0
    } else {
        current_hwnd
    };

    let mut reference_changed = false;
    let (current_reference, lock_enabled, put_in_tray) = {
        let mut state = tracking_state()
            .lock()
            .map_err(|_| "追従状態のロックに失敗しました".to_string())?;

        if state.reference_hwnd != 0 && !is_valid_window(state.reference_hwnd) {
            state.reference_hwnd = 0;
            state.pending_hwnd = 0;
            state.pending_count = 0;
            state.lock_enabled = false;
            reference_changed = true;
        }

        if state.lock_enabled {
            if state.reference_hwnd == 0 && candidate_hwnd != 0 {
                state.reference_hwnd = candidate_hwnd;
                reference_changed = true;
            }
        } else if candidate_hwnd != 0 {
            if state.reference_hwnd != candidate_hwnd {
                if state.pending_hwnd == candidate_hwnd {
                    state.pending_count += 1;
                } else {
                    state.pending_hwnd = candidate_hwnd;
                    state.pending_count = 1;
                }

                if state.pending_count >= REFERENCE_WND_CHANGE_FRAME_COUNT {
                    state.reference_hwnd = candidate_hwnd;
                    state.pending_hwnd = 0;
                    state.pending_count = 0;
                    reference_changed = true;
                }
            } else {
                state.pending_hwnd = 0;
                state.pending_count = 0;
            }
        }

        (state.reference_hwnd, state.lock_enabled, state.put_in_tray)
    };

    if put_in_tray {
        let mut should_emit_visible = false;
        {
            let mut state = tracking_state()
                .lock()
                .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
            if state.last_visible {
                state.last_visible = false;
                should_emit_visible = true;
            }
        }
        window.hide().map_err(|err| err.to_string())?;
        if should_emit_visible {
            window
                .emit(
                    "launcher://window-tracked",
                    serde_json::json!({ "x": 0, "y": 0, "width": 0, "visible": false }),
                )
                .map_err(|err| err.to_string())?;
        }
        return Ok(());
    }

    if current_reference == 0 {
        let mut should_emit_visible = false;
        {
            let mut state = tracking_state()
                .lock()
                .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
            if state.last_visible {
                state.last_visible = false;
                should_emit_visible = true;
            }
        }

        window.hide().map_err(|err| err.to_string())?;
        if should_emit_visible {
            window
                .emit(
                    "launcher://window-tracked",
                    serde_json::json!({ "x": 0, "y": 0, "width": 0, "visible": false }),
                )
                .map_err(|err| err.to_string())?;
        }
        if reference_changed {
            emit_reference_changed(&window, current_reference, lock_enabled)?;
        }
        return Ok(());
    }

    let Some(rect) = get_window_rect(current_reference) else {
        let mut state = tracking_state()
            .lock()
            .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
        state.reference_hwnd = 0;
        state.pending_hwnd = 0;
        state.pending_count = 0;
        state.lock_enabled = false;
        return Ok(());
    };

    let tracked_width = (rect.right - rect.left + 1).max(1);
    let top = rect.top - BAR_HEIGHT;

    window
        .set_size(Size::Physical(PhysicalSize::new(
            tracked_width as u32,
            BAR_HEIGHT as u32,
        )))
        .map_err(|err| err.to_string())?;
    window
        .set_position(Position::Physical(PhysicalPosition::new(rect.left, top)))
        .map_err(|err| err.to_string())?;
    window
        .set_always_on_top(!lock_enabled)
        .map_err(|err| err.to_string())?;
    window.show().map_err(|err| err.to_string())?;

    {
        let mut state = tracking_state()
            .lock()
            .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
        state.last_visible = true;
    }

    window
        .emit(
            "launcher://window-tracked",
            serde_json::json!({
                "x": rect.left,
                "y": top,
                "width": tracked_width,
                "visible": true
            }),
        )
        .map_err(|err| err.to_string())?;

    if reference_changed {
        emit_reference_changed(&window, current_reference, lock_enabled)?;
    }

    Ok(())
}

fn ensure_tracking_thread(app: AppHandle) -> Result<(), String> {
    {
        let mut state = tracking_state()
            .lock()
            .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
        if state.thread_started {
            return Ok(());
        }
        state.thread_started = true;
    }

    thread::spawn(move || loop {
        if let Err(message) = update_tracking(&app) {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit(
                    "launcher://tracking-error",
                    serde_json::json!({ "message": message }),
                );
            }
        }
        thread::sleep(Duration::from_millis(TRACK_INTERVAL_MILLIS));
    });

    Ok(())
}

#[tauri::command]
pub fn launcher_init(app: AppHandle) -> Result<(), String> {
    let _ = load_settings(&app)?;
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_decorations(false)
            .map_err(|err| err.to_string())?;
        window.set_resizable(false).map_err(|err| err.to_string())?;
    }
    ensure_tracking_thread(app)
}

#[tauri::command]
pub fn launcher_start_tracking(app: AppHandle) -> Result<(), String> {
    let (reference_hwnd, lock_enabled) = {
        let mut state = tracking_state()
            .lock()
            .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
        state.lock_enabled = true;
        (state.reference_hwnd, state.lock_enabled)
    };

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main ウインドウが見つかりません".to_string())?;
    emit_reference_changed(&window, reference_hwnd, lock_enabled)
}

#[tauri::command]
pub fn launcher_stop_tracking(app: AppHandle) -> Result<(), String> {
    let (reference_hwnd, lock_enabled) = {
        let mut state = tracking_state()
            .lock()
            .map_err(|_| "追従状態のロックに失敗しました".to_string())?;
        state.lock_enabled = false;
        (state.reference_hwnd, state.lock_enabled)
    };

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main ウインドウが見つかりません".to_string())?;
    emit_reference_changed(&window, reference_hwnd, lock_enabled)
}

#[tauri::command]
pub fn launcher_hide_to_tray(app: AppHandle) -> Result<(), String> {
    set_put_in_tray(true)?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main ウインドウが見つかりません".to_string())?;
    window.hide().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn launcher_restore_from_tray(app: AppHandle) -> Result<(), String> {
    set_put_in_tray(false)?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main ウインドウが見つかりません".to_string())?;
    window.show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn launcher_open_slot_editor(app: AppHandle, index: usize) -> Result<(), String> {
    let settings = load_settings(&app)?;
    if index >= settings.slots.len() {
        return Err("スロット番号が範囲外です".to_string());
    }

    let existing_labels: Vec<String> = app
        .webview_windows()
        .keys()
        .filter(|label| label.starts_with("slot-editor"))
        .cloned()
        .collect();
    for label in existing_labels {
        if let Some(window) = app.get_webview_window(&label) {
            window.close().map_err(|err| err.to_string())?;
        }
    }

    let label = format!("slot-editor-{index}");
    let editor = WebviewWindowBuilder::new(&app, label, WebviewUrl::App("slot-edit".into()))
        .title(format!("ショートカット設定 {}", index + 1))
        .inner_size(560.0, 440.0)
        .resizable(false)
        .center()
        .build()
        .map_err(|err| err.to_string())?;

    editor.set_focus().map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_load(app: AppHandle) -> Result<LauncherSettings, String> {
    load_settings(&app)
}

#[tauri::command]
pub fn settings_save(app: AppHandle, settings: LauncherSettings) -> Result<(), String> {
    write_settings(&app, &settings)?;
    emit_settings_updated(&app)
}

#[tauri::command]
pub fn slot_swap(app: AppHandle, lhs: usize, rhs: usize) -> Result<LauncherSettings, String> {
    let mut settings = load_settings(&app)?;
    if lhs >= settings.slots.len() || rhs >= settings.slots.len() {
        return Err("スロット番号が範囲外です".to_string());
    }
    settings.slots.swap(lhs, rhs);
    write_settings(&app, &settings)?;
    emit_settings_updated(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn slot_update(app: AppHandle, slot: LauncherSlot) -> Result<LauncherSettings, String> {
    let mut settings = load_settings(&app)?;
    if slot.index >= settings.slots.len() {
        return Err("スロット番号が範囲外です".to_string());
    }
    settings.slots[slot.index] = slot.clone();
    settings.slots[slot.index].exist = disk_path_exists(&slot.path, &slot.data_type);
    write_settings(&app, &settings)?;
    emit_settings_updated(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn slot_clear(app: AppHandle, index: usize) -> Result<LauncherSettings, String> {
    let mut settings = load_settings(&app)?;
    if index >= settings.slots.len() {
        return Err("スロット番号が範囲外です".to_string());
    }
    settings.slots[index] = LauncherSlot {
        index,
        data_type: DataType::None,
        path: String::new(),
        arg: String::new(),
        comment: String::new(),
        exist: false,
    };
    write_settings(&app, &settings)?;
    emit_settings_updated(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn slot_execute(
    app: AppHandle,
    index: usize,
    dropped_arg: Option<String>,
) -> Result<(), String> {
    let settings = load_settings(&app)?;
    let slot = settings
        .slots
        .get(index)
        .ok_or_else(|| "スロットが存在しません".to_string())?;
    if slot.data_type == DataType::None || !slot.exist {
        return Err("実行可能な対象がありません".to_string());
    }

    let command_arg = dropped_arg.filter(|value| !value.trim().is_empty());
    let target = slot.path.as_str();

    let mut command = match &slot.data_type {
        DataType::Folder => {
            let mut cmd = Command::new("explorer");
            cmd.arg(target);
            cmd
        }
        DataType::Url => {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", "start", "", target]);
            cmd
        }
        DataType::Cpl => {
            let mut cmd = Command::new("control");
            cmd.arg(target);
            cmd
        }
        DataType::Script => {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", target]);
            if let Some(arg) = command_arg.as_deref() {
                cmd.arg(arg);
            }
            cmd
        }
        _ => {
            let mut cmd = Command::new(target);
            if let Some(arg) = command_arg.as_deref() {
                cmd.arg(arg);
            }
            if !slot.arg.trim().is_empty() {
                cmd.arg(&slot.arg);
            }
            cmd
        }
    };

    command.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn path_detect_data_type(path: String) -> Result<DataType, String> {
    Ok(detect_data_type(&path))
}

#[tauri::command]
pub fn path_exists(path: String, data_type: DataType) -> Result<bool, String> {
    Ok(disk_path_exists(&path, &data_type))
}
