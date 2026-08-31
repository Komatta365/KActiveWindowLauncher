use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Window};

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
    pub slots: Vec<LauncherSlot>,
}

fn default_settings() -> LauncherSettings {
    let slots = (0..64)
        .map(|index| LauncherSlot {
            index,
            data_type: DataType::None,
            path: String::new(),
            arg: String::new(),
            comment: String::new(),
            exist: false,
        })
        .collect();

    LauncherSettings { version: 1, slots }
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
                && path.file_name().is_some_and(|name| name.to_string_lossy().contains("settings_backup_"))
        })
        .collect::<Vec<_>>();

    files.sort_by(|lhs, rhs| file_modified_time(rhs).cmp(&file_modified_time(lhs)));
    files
}

fn write_settings(app: &AppHandle, settings: &LauncherSettings) -> Result<(), String> {
    let file_path = settings_file_path(app).ok_or_else(|| "設定ファイルの保存先を決定できませんでした".to_string())?;
    let contents = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
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
        let old = backups.pop().unwrap();
        let _ = fs::remove_file(old);
    }
    Ok(())
}

fn load_settings(app: &AppHandle) -> Result<LauncherSettings, String> {
    let main_path = settings_file_path(app).ok_or_else(|| "設定ファイルの場所が確認できません".to_string())?;
    if main_path.exists() {
        let bytes = fs::read(&main_path).map_err(|err| err.to_string())?;
        if !bytes.is_empty() {
            if let Ok(settings) = serde_json::from_slice::<LauncherSettings>(&bytes) {
                return Ok(settings);
            }
        }
    }

    let mut backups = backup_file_paths(app);
    backups.sort_by(|lhs, rhs| file_modified_time(rhs).cmp(&file_modified_time(lhs)));
    for backup in backups {
        if let Ok(content) = fs::read(&backup) {
            if !content.is_empty() {
                if let Ok(settings) = serde_json::from_slice::<LauncherSettings>(&content) {
                    return Ok(settings);
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

    match path_ref.extension().and_then(|ext| ext.to_str()).map(|value| value.to_ascii_lowercase()) {
        Some(ext) if ext == "exe" => DataType::Exe,
        Some(ext) if ext == "vbs" || ext == "bat" => DataType::Script,
        Some(ext) if matches!(ext.as_str(), "bmp" | "jpg" | "jpe" | "jpeg" | "png" | "tga") => DataType::Image,
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
        DataType::Url => candidate.contains("http://") || candidate.contains("https://") || candidate.starts_with("www"),
        _ => target.exists(),
    }
}

#[tauri::command]
pub fn launcher_init(app: AppHandle) -> Result<(), String> {
    let _ = load_settings(&app)?;
    Ok(())
}

#[tauri::command]
pub fn launcher_start_tracking(window: Window) -> Result<(), String> {
    window.emit("launcher://reference-window-changed", serde_json::json!({ "hwnd": null, "locked": false })).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn launcher_stop_tracking(window: Window) -> Result<(), String> {
    window.emit("launcher://window-tracked", serde_json::json!({ "x": 0, "y": 0, "width": 0, "visible": false })).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_load(app: AppHandle) -> Result<LauncherSettings, String> {
    load_settings(&app)
}

#[tauri::command]
pub fn settings_save(app: AppHandle, settings: LauncherSettings) -> Result<(), String> {
    write_settings(&app, &settings)
}

#[tauri::command]
pub fn slot_swap(app: AppHandle, lhs: usize, rhs: usize) -> Result<LauncherSettings, String> {
    let mut settings = load_settings(&app)?;
    if lhs >= settings.slots.len() || rhs >= settings.slots.len() {
        return Err("スロット番号が範囲外です".to_string());
    }
    settings.slots.swap(lhs, rhs);
    write_settings(&app, &settings)?;
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
    Ok(settings)
}

#[tauri::command]
pub fn slot_execute(app: AppHandle, index: usize, dropped_arg: Option<String>) -> Result<(), String> {
    let settings = load_settings(&app)?;
    let slot = settings.slots.get(index).ok_or_else(|| "スロットが存在しません".to_string())?;
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
