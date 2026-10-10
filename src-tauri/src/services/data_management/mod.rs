use chrono::Local;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::services::database::connection::{close_database, with_connection};
use crate::services::database::init_database;
use crate::services::settings::storage::SettingsStorage;
use crate::services::system::hotkey::reload_from_settings;
use crate::services::{get_data_directory, get_settings, update_settings};

#[derive(Debug, Clone, Serialize)]
pub struct TargetDataInfo {
    pub has_data: bool,
    pub has_database: bool,
    pub has_images: bool,
    pub has_image_library: bool,
    pub database_size: u64,
    pub images_count: usize,
    pub images_size: u64,
    pub image_library_count: usize,
    pub image_library_size: u64,
}

pub fn check_target_has_data(target_dir: &Path) -> Result<TargetDataInfo, String> {
    let db_path = target_dir.join("quickclipboard.db");
    let images_dir = target_dir.join("clipboard_images");
    let image_library_dir = target_dir.join("image_library");

    let has_database = db_path.exists();
    let database_size = if has_database {
        fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let (has_images, images_count, images_size) = if images_dir.exists() {
        let mut count = 0usize;
        let mut size = 0u64;
        if let Ok(entries) = fs::read_dir(&images_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        count += 1;
                        size += meta.len();
                    }
                }
            }
        }
        (count > 0, count, size)
    } else {
        (false, 0, 0)
    };

    let (has_image_library, image_library_count, image_library_size) = if image_library_dir.exists()
    {
        let mut count = 0usize;
        let mut size = 0u64;
        if let Ok(entries) = fs::read_dir(&image_library_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(sub_entries) = fs::read_dir(&path) {
                        for sub_entry in sub_entries.flatten() {
                            if let Ok(meta) = sub_entry.metadata() {
                                if meta.is_file() {
                                    count += 1;
                                    size += meta.len();
                                }
                            }
                        }
                    }
                } else if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        count += 1;
                        size += meta.len();
                    }
                }
            }
        }
        (count > 0, count, size)
    } else {
        (false, 0, 0)
    };

    Ok(TargetDataInfo {
        has_data: has_database || has_images || has_image_library,
        has_database,
        has_images,
        has_image_library,
        database_size,
        images_count,
        images_size,
        image_library_count,
        image_library_size,
    })
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| format!("复制文件失败: {}", e))?;
        }
    }
    Ok(())
}

fn backup_full_zip(dir: &Path) -> Result<Option<PathBuf>, String> {
    let db = dir.join("quickclipboard.db");
    let images_dir = dir.join("clipboard_images");
    let app_icons_dir = dir.join("app_icons");
    if !db.exists() && !images_dir.exists() {
        return Ok(None);
    }

    let backups = dir.join("backups");
    fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let ts_str = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = format!("quickclipboard-backup-{}.zip", ts_str);
    let target = backups.join(&name);

    let file = fs::File::create(&target).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    if db.exists() {
        let mut f = fs::File::open(&db).map_err(|e| e.to_string())?;
        zip.start_file("quickclipboard.db", options)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
    }

    if images_dir.exists() {
        for entry in fs::read_dir(&images_dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    let zip_path = format!("clipboard_images/{}", fname);
                    let mut f = fs::File::open(&path).map_err(|e| e.to_string())?;
                    zip.start_file(&zip_path, options)
                        .map_err(|e| e.to_string())?;
                    std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
                }
            }
        }
    }

    if app_icons_dir.exists() {
        for entry in fs::read_dir(&app_icons_dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    let zip_path = format!("app_icons/{}", fname);
                    let mut f = fs::File::open(&path).map_err(|e| e.to_string())?;
                    zip.start_file(&zip_path, options)
                        .map_err(|e| e.to_string())?;
                    std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
                }
            }
        }
    }

    if let Ok(settings_path) = SettingsStorage::get_settings_path() {
        if settings_path.exists() {
            if let Ok(mut f) = fs::File::open(&settings_path) {
                let _ = zip.start_file("settings.json", options);
                let _ = std::io::copy(&mut f, &mut zip);
            }
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    enforce_backup_retention(&backups, 10)?;
    Ok(Some(target))
}

// 获取备份列表
#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub created_at: String,
}

pub fn list_backups() -> Result<Vec<BackupInfo>, String> {
    let current_dir = get_current_storage_dir()?;
    let backups_dir = current_dir.join("backups");
    if !backups_dir.exists() {
        return Ok(vec![]);
    }

    let mut items: Vec<BackupInfo> = Vec::new();
    for e in fs::read_dir(&backups_dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let fname = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !fname.starts_with("quickclipboard-backup-") || !fname.ends_with(".zip") {
            continue;
        }
        let md = e.metadata().map_err(|e| e.to_string())?;
        let size = md.len();
        let modified = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let datetime: chrono::DateTime<Local> = modified.into();
        items.push(BackupInfo {
            path: p.to_string_lossy().to_string(),
            name: fname,
            size,
            created_at: datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
        });
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

fn enforce_backup_retention(backups_dir: &Path, keep: usize) -> Result<(), String> {
    let mut items: Vec<(SystemTime, PathBuf)> = Vec::new();
    for e in fs::read_dir(backups_dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !fname.starts_with("quickclipboard-backup-") || !fname.ends_with(".zip") {
            continue;
        }
        let md = e.metadata().map_err(|e| e.to_string())?;
        let t = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        items.push((t, p));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    if items.len() > keep {
        for (_, p) in items.into_iter().skip(keep) {
            let _ = fs::remove_file(p);
        }
    }
    Ok(())
}

pub fn reset_all_data() -> Result<String, String> {
    let current_dir = get_current_storage_dir()?;
    let default_dir = get_default_data_dir()?;

    let _ = crate::services::database::connection::with_connection(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(FULL); PRAGMA wal_checkpoint(TRUNCATE);")
    });
    let _ = backup_full_zip(&current_dir);
    if current_dir != default_dir {
        let _ = backup_full_zip(&default_dir);
    }

    close_database();

    fn clean_dir(dir: &Path) -> Result<(), String> {
        let images = dir.join("clipboard_images");
        if images.exists() {
            let _ = fs::remove_dir_all(&images);
        }
        let image_library = dir.join("image_library");
        if image_library.exists() {
            let _ = fs::remove_dir_all(&image_library);
        }
        let app_icons = dir.join("app_icons");
        if app_icons.exists() {
            let _ = fs::remove_dir_all(&app_icons);
        }
        for name in [
            "quickclipboard.db",
            "quickclipboard.db-shm",
            "quickclipboard.db-wal",
        ] {
            let p = dir.join(name);
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }
        Ok(())
    }

    clean_dir(&current_dir)?;
    if current_dir != default_dir {
        clean_dir(&default_dir)?;
    }

    let mut defaults = crate::services::AppSettings::default();
    defaults.use_custom_storage = false;
    defaults.custom_storage_path = None;
    update_settings(defaults.clone())?;

    let db_path = default_dir.join("quickclipboard.db");
    init_database(db_path.to_str().ok_or("数据库路径无效")?)?;
    let _ = crate::services::database::connection::with_connection(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
    });

    let _ = reload_from_settings();

    Ok(default_dir.to_string_lossy().to_string())
}

pub fn import_data_zip(zip_path: PathBuf, mode: &str) -> Result<String, String> {
    if !zip_path.exists() {
        return Err("导入文件不存在".into());
    }

    let temp_root =
        std::env::temp_dir().join(format!("quickclipboard_import_{}", fastrand::u32(..)));
    fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;
    let file = fs::File::open(&zip_path).map_err(|e| format!("打开导入文件失败: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("读取压缩包失败: {}", e))?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if name.contains("..") {
            continue;
        }
        let out_path = temp_root.join(&name);
        if f.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out_path.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        }
    }

    let imported_db = temp_root.join("quickclipboard.db");
    let imported_images = temp_root.join("clipboard_images");
    let imported_image_library = temp_root.join("image_library");
    let imported_app_icons = temp_root.join("app_icons");
    let imported_settings = temp_root.join("settings.json");

    match mode {
        "replace" => {
            let current_dir_for_backup = get_current_storage_dir()?;
            let _ = crate::services::database::connection::with_connection(|conn| {
                conn.execute_batch("PRAGMA wal_checkpoint(FULL); PRAGMA wal_checkpoint(TRUNCATE);")
            });
            let _ = backup_full_zip(&current_dir_for_backup);
            let mut new_settings = if imported_settings.exists() {
                let s = fs::read_to_string(&imported_settings).map_err(|e| e.to_string())?;
                serde_json::from_str::<crate::services::AppSettings>(&s)
                    .map_err(|e| e.to_string())?
            } else {
                get_settings()
            };
            if crate::services::is_portable_build()
                || std::env::current_exe()
                    .ok()
                    .and_then(|e| e.parent().map(|p| p.join("portable.txt").exists()))
                    .unwrap_or(false)
            {
                new_settings.use_custom_storage = false;
                new_settings.custom_storage_path = None;
            }

            let target_dir = if new_settings.use_custom_storage {
                if let Some(ref path) = new_settings.custom_storage_path {
                    let p = PathBuf::from(path);
                    if p.exists() {
                        p
                    } else {
                        new_settings.use_custom_storage = false;
                        new_settings.custom_storage_path = None;
                        get_default_data_dir()?
                    }
                } else {
                    get_default_data_dir()?
                }
            } else {
                get_default_data_dir()?
            };

            update_settings(new_settings.clone())?;

            close_database();

            let target_images = target_dir.join("clipboard_images");
            if target_images.exists() {
                fs::remove_dir_all(&target_images).map_err(|e| e.to_string())?;
            }
            if imported_images.exists() {
                copy_dir_all(&imported_images, &target_images)?;
            }
            let target_image_library = target_dir.join("image_library");
            if target_image_library.exists() {
                fs::remove_dir_all(&target_image_library).map_err(|e| e.to_string())?;
            }
            if imported_image_library.exists() {
                copy_dir_all(&imported_image_library, &target_image_library)?;
            }
            let target_app_icons = target_dir.join("app_icons");
            if target_app_icons.exists() {
                fs::remove_dir_all(&target_app_icons).map_err(|e| e.to_string())?;
            }
            if imported_app_icons.exists() {
                copy_dir_all(&imported_app_icons, &target_app_icons)?;
            }

            let src_db = temp_root.join("quickclipboard.db");
            let dst_db = target_dir.join("quickclipboard.db");
            if src_db.exists() {
                if let Some(p) = dst_db.parent() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                fs::copy(&src_db, &dst_db).map_err(|e| e.to_string())?;
            }
            for name in ["quickclipboard.db-shm", "quickclipboard.db-wal"] {
                let p = target_dir.join(name);
                if p.exists() {
                    let _ = fs::remove_file(&p);
                }
            }

            let db_path = target_dir.join("quickclipboard.db");
            init_database(db_path.to_str().ok_or("数据库路径无效")?)?;
            let _ = crate::services::database::connection::with_connection(|conn| {
                conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            });

            let _ = reload_from_settings();

            let _ = fs::remove_dir_all(&temp_root);
            Ok(target_dir.to_string_lossy().to_string())
        }
        "merge" => {
            let current_dir = get_current_storage_dir()?;

            let target_images = current_dir.join("clipboard_images");
            if imported_images.exists() {
                if !target_images.exists() {
                    fs::create_dir_all(&target_images).map_err(|e| e.to_string())?;
                }
                merge_dir_overwrite(&imported_images, &target_images)?;
            }

            let target_image_library = current_dir.join("image_library");
            if imported_image_library.exists() {
                if !target_image_library.exists() {
                    fs::create_dir_all(&target_image_library).map_err(|e| e.to_string())?;
                }
                merge_dir_overwrite(&imported_image_library, &target_image_library)?;
            }

            let target_app_icons = current_dir.join("app_icons");
            if imported_app_icons.exists() {
                if !target_app_icons.exists() {
                    fs::create_dir_all(&target_app_icons).map_err(|e| e.to_string())?;
                }
                merge_dir_overwrite(&imported_app_icons, &target_app_icons)?;
            }

            if imported_db.exists() {
                merge_database(&imported_db)?;
            }

            let _ = fs::remove_dir_all(&temp_root);
            Ok(current_dir.to_string_lossy().to_string())
        }
        _ => {
            let _ = fs::remove_dir_all(&temp_root);
            Err("不支持的导入模式".into())
        }
    }
}
fn safe_move_item(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if fs::rename(src, dst).is_err() {
        if src.is_dir() {
            copy_dir_all(src, dst)?;
            fs::remove_dir_all(src).map_err(|e| format!("删除源目录失败: {}", e))?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(src, dst).map_err(|e| format!("复制文件失败: {}", e))?;
            fs::remove_file(src).map_err(|e| format!("删除源文件失败: {}", e))?;
        }
    }
    Ok(())
}

pub fn get_default_data_dir() -> Result<PathBuf, String> {
    let settings_path = SettingsStorage::get_settings_path()?;
    settings_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("无法获取默认数据目录".to_string())
}

pub fn get_current_storage_dir() -> Result<PathBuf, String> {
    get_data_directory()
}

// mode: "source_only" | "target_only" | "merge"
pub fn change_storage_dir(new_dir: PathBuf, mode: &str) -> Result<PathBuf, String> {
    if crate::services::is_portable_build()
        || std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("portable.txt").exists()))
            .unwrap_or(false)
    {
        return Err("便携版不支持更改存储路径".into());
    }
    if !new_dir.exists() {
        fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;
    }

    let current_dir = get_current_storage_dir()?;
    if new_dir == current_dir {
        return Err("新位置与当前存储位置相同，无需迁移".to_string());
    }

    change_storage_dir_internal(&current_dir, &new_dir, mode)?;

    let mut settings = get_settings();
    settings.use_custom_storage = true;
    settings.custom_storage_path = Some(new_dir.to_string_lossy().to_string());
    update_settings(settings.clone())?;

    let db_path = new_dir.join("quickclipboard.db");
    init_database(db_path.to_str().ok_or("数据库路径无效")?)?;
    let _ = crate::services::database::connection::with_connection(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
    });

    Ok(new_dir)
}

fn merge_dir_overwrite(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|e| e.to_string())?;
            merge_dir_overwrite(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn merge_dir_no_overwrite(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            if !dst_path.exists() {
                fs::create_dir_all(&dst_path).map_err(|e| e.to_string())?;
            }
            merge_dir_no_overwrite(&src_path, &dst_path)?;
        } else if !dst_path.exists() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&src_path, &dst_path).map_err(|e| format!("复制文件失败: {}", e))?;
        }
    }
    Ok(())
}

fn merge_database(src_db: &Path) -> Result<(), String> {
    with_connection(|conn| {
        let import_path = src_db
            .to_str()
            .ok_or(rusqlite::Error::InvalidPath("bad path".into()))?;
        conn.execute("ATTACH DATABASE ?1 AS importdb", [import_path])?;

        let merge_result = (|| -> rusqlite::Result<()> {
            merge_groups_from_importdb(conn)?;
            merge_favorites_from_importdb(conn)?;

            // 记录导入库 clipboard.id 到当前库新 id 的映射，用于迁移 clipboard_data。
            let id_mapping = merge_clipboard_from_importdb(conn)?;
            merge_clipboard_data_from_importdb(conn, &id_mapping)?;

            reorder_clipboard_by_time(conn);
            Ok(())
        })();

        let _ = conn.execute("DETACH DATABASE importdb", []);
        merge_result
    })?;
    Ok(())
}

fn importdb_has_table(conn: &rusqlite::Connection, table: &str) -> rusqlite::Result<bool> {
    let sql = "SELECT COUNT(*) FROM importdb.sqlite_master WHERE type = 'table' AND name = ?1";
    let count: i64 = conn.query_row(sql, [table], |row| row.get(0))?;
    Ok(count > 0)
}

fn importdb_table_columns(
    conn: &rusqlite::Connection,
    table: &str,
) -> rusqlite::Result<std::collections::HashSet<String>> {
    let pragma_sql = format!("PRAGMA importdb.table_info({})", table);
    let mut stmt = conn.prepare(&pragma_sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = std::collections::HashSet::new();
    for col in rows {
        columns.insert(col?);
    }
    Ok(columns)
}

fn has_col(columns: &std::collections::HashSet<String>, name: &str) -> bool {
    columns.contains(name)
}

fn merge_groups_from_importdb(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    if !importdb_has_table(conn, "groups")? {
        return Ok(());
    }

    let cols = importdb_table_columns(conn, "groups")?;
    let icon_expr = if has_col(&cols, "icon") {
        "icon"
    } else {
        "'ti ti-folder'"
    };
    let color_expr = if has_col(&cols, "color") {
        "color"
    } else {
        "'#dc2626'"
    };
    let order_expr = if has_col(&cols, "order_index") {
        "order_index"
    } else {
        "0"
    };
    let created_expr = if has_col(&cols, "created_at") {
        "created_at"
    } else {
        "CAST(strftime('%s','now') AS INTEGER)"
    };
    let updated_expr = if has_col(&cols, "updated_at") {
        "updated_at"
    } else {
        "CAST(strftime('%s','now') AS INTEGER)"
    };

    let sql = format!(
        "INSERT OR IGNORE INTO groups (name, icon, color, order_index, created_at, updated_at)
         SELECT name, {icon}, {color}, {order_index}, {created_at}, {updated_at}
         FROM importdb.groups",
        icon = icon_expr,
        color = color_expr,
        order_index = order_expr,
        created_at = created_expr,
        updated_at = updated_expr
    );
    let _ = conn.execute(&sql, []);
    Ok(())
}

fn merge_favorites_from_importdb(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    if !importdb_has_table(conn, "favorites")? {
        return Ok(());
    }

    let cols = importdb_table_columns(conn, "favorites")?;
    let html_expr = if has_col(&cols, "html_content") {
        "html_content"
    } else {
        "NULL"
    };
    let content_type_expr = if has_col(&cols, "content_type") {
        "content_type"
    } else {
        "'text'"
    };
    let image_id_expr = if has_col(&cols, "image_id") {
        "image_id"
    } else {
        "NULL"
    };
    let group_expr = if has_col(&cols, "group_name") {
        "group_name"
    } else {
        "'全部'"
    };
    let item_order_expr = if has_col(&cols, "item_order") {
        "item_order"
    } else {
        "0"
    };
    let created_expr = if has_col(&cols, "created_at") {
        "created_at"
    } else {
        "CAST(strftime('%s','now') AS INTEGER)"
    };
    let updated_expr = if has_col(&cols, "updated_at") {
        "updated_at"
    } else {
        "CAST(strftime('%s','now') AS INTEGER)"
    };
    let paste_count_expr = if has_col(&cols, "paste_count") {
        "paste_count"
    } else {
        "0"
    };
    let char_count_expr = if has_col(&cols, "char_count") {
        "char_count"
    } else {
        "NULL"
    };

    let sql = format!(
        "INSERT OR IGNORE INTO favorites
         (id, title, content, html_content, content_type, image_id, group_name, item_order, paste_count, char_count, created_at, updated_at)
         SELECT
            id,
            title,
            content,
            {html},
            {content_type},
            {image_id},
            {group_name},
            {item_order},
            {paste_count},
            {char_count},
            {created_at},
            {updated_at}
         FROM importdb.favorites",
        html = html_expr,
        content_type = content_type_expr,
        image_id = image_id_expr,
        group_name = group_expr,
        item_order = item_order_expr,
        paste_count = paste_count_expr,
        char_count = char_count_expr,
        created_at = created_expr,
        updated_at = updated_expr
    );
    let _ = conn.execute(&sql, []);
    Ok(())
}

fn merge_clipboard_from_importdb(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<std::collections::HashMap<i64, i64>> {
    let mut id_map = HashMap::new();
    if !importdb_has_table(conn, "clipboard")? {
        return Ok(id_map);
    }

    let cols = importdb_table_columns(conn, "clipboard")?;
    let id_expr = if has_col(&cols, "id") { "id" } else { "NULL" };
    let html_expr = if has_col(&cols, "html_content") {
        "html_content"
    } else {
        "NULL"
    };
    let content_type_expr = if has_col(&cols, "content_type") {
        "content_type"
    } else {
        "'text'"
    };
    let image_id_expr = if has_col(&cols, "image_id") {
        "image_id"
    } else {
        "NULL"
    };
    let is_pinned_expr = if has_col(&cols, "is_pinned") {
        "is_pinned"
    } else {
        "0"
    };
    let paste_count_expr = if has_col(&cols, "paste_count") {
        "paste_count"
    } else {
        "0"
    };
    let source_app_expr = if has_col(&cols, "source_app") {
        "source_app"
    } else {
        "NULL"
    };
    let source_icon_hash_expr = if has_col(&cols, "source_icon_hash") {
        "source_icon_hash"
    } else {
        "NULL"
    };
    let char_count_expr = if has_col(&cols, "char_count") {
        "char_count"
    } else {
        "NULL"
    };
    let uuid_expr = if has_col(&cols, "uuid") {
        "uuid"
    } else {
        "NULL"
    };
    let source_device_id_expr = if has_col(&cols, "source_device_id") {
        "source_device_id"
    } else {
        "NULL"
    };
    let is_remote_expr = if has_col(&cols, "is_remote") {
        "is_remote"
    } else {
        "0"
    };
    let created_expr = if has_col(&cols, "created_at") {
        "created_at"
    } else {
        "CAST(strftime('%s','now') AS INTEGER)"
    };
    let updated_expr = if has_col(&cols, "updated_at") {
        "updated_at"
    } else {
        "CAST(strftime('%s','now') AS INTEGER)"
    };

    let select_sql = format!(
        "SELECT
            {id},
            content,
            {html},
            {content_type},
            {image_id},
            {is_pinned},
            {paste_count},
            {source_app},
            {source_icon_hash},
            {char_count},
            {uuid},
            {source_device_id},
            {is_remote},
            {created_at},
            {updated_at}
         FROM importdb.clipboard",
        id = id_expr,
        html = html_expr,
        content_type = content_type_expr,
        image_id = image_id_expr,
        is_pinned = is_pinned_expr,
        paste_count = paste_count_expr,
        source_app = source_app_expr,
        source_icon_hash = source_icon_hash_expr,
        char_count = char_count_expr,
        uuid = uuid_expr,
        source_device_id = source_device_id_expr,
        is_remote = is_remote_expr,
        created_at = created_expr,
        updated_at = updated_expr
    );

    let mut query_stmt = conn.prepare(&select_sql)?;
    let mut insert_stmt = conn.prepare(
        "INSERT INTO clipboard
         (content, html_content, content_type, image_id, is_pinned, paste_count, source_app, source_icon_hash, char_count, uuid, source_device_id, is_remote, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?;

    let rows = query_stmt.query_map([], |row| {
        Ok((
            row.get::<_, Option<i64>>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<i64>>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, i64>(12)?,
            row.get::<_, i64>(13)?,
            row.get::<_, i64>(14)?,
        ))
    })?;

    for row in rows {
        let (
            old_id,
            content,
            html_content,
            content_type,
            image_id,
            is_pinned,
            paste_count,
            source_app,
            source_icon_hash,
            char_count,
            uuid,
            source_device_id,
            is_remote,
            created_at,
            updated_at,
        ) = row?;

        insert_stmt.execute(rusqlite::params![
            content,
            html_content,
            content_type,
            image_id,
            is_pinned,
            paste_count,
            source_app,
            source_icon_hash,
            char_count,
            uuid,
            source_device_id,
            is_remote,
            created_at,
            updated_at
        ])?;

        if let Some(old_id) = old_id {
            id_map.insert(old_id, conn.last_insert_rowid());
        }
    }

    Ok(id_map)
}

fn merge_clipboard_data_from_importdb(
    conn: &rusqlite::Connection,
    clipboard_id_map: &std::collections::HashMap<i64, i64>,
) -> rusqlite::Result<()> {
    if !importdb_has_table(conn, "clipboard_data")? {
        return Ok(());
    }

    // 收藏原始格式：目标 id 不变，合并时不覆盖现有格式。
    let _ = conn.execute(
        "INSERT OR IGNORE INTO clipboard_data
         (target_kind, target_id, format_name, raw_data, is_primary, format_order, created_at, updated_at)
         SELECT d.target_kind, d.target_id, d.format_name, d.raw_data, d.is_primary, d.format_order, d.created_at, d.updated_at
         FROM importdb.clipboard_data d
         WHERE d.target_kind = 'favorite' AND EXISTS (SELECT 1 FROM favorites f WHERE f.id = d.target_id)",
        [],
    );

    if clipboard_id_map.is_empty() {
        return Ok(());
    }

    let mut source_stmt = conn.prepare(
        "SELECT format_name, raw_data, is_primary, format_order, created_at, updated_at
         FROM importdb.clipboard_data
         WHERE target_kind = 'clipboard' AND target_id = ?1
         ORDER BY format_order, id",
    )?;
    let mut insert_stmt = conn.prepare(
        "INSERT OR IGNORE INTO clipboard_data
         (target_kind, target_id, format_name, raw_data, is_primary, format_order, created_at, updated_at)
         VALUES ('clipboard', ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for (old_id, new_id) in clipboard_id_map {
        let old_target = old_id.to_string();
        let rows = source_stmt.query_map([old_target], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;

        for row in rows {
            let (format_name, raw_data, is_primary, format_order, created_at, updated_at) = row?;
            insert_stmt.execute(rusqlite::params![
                new_id.to_string(),
                format_name,
                raw_data,
                is_primary,
                format_order,
                created_at,
                updated_at
            ])?;
        }
    }

    Ok(())
}

fn reorder_clipboard_by_time(conn: &rusqlite::Connection) {
    if let Ok(mut stmt) =
        conn.prepare("SELECT id FROM clipboard ORDER BY is_pinned DESC, created_at DESC")
    {
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        let count = ids.len() as i64;
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE clipboard SET item_order = ? WHERE id = ?",
                rusqlite::params![count - i as i64, id],
            )
            .ok();
        }
    }
}

pub fn reset_storage_dir_to_default(mode: &str) -> Result<PathBuf, String> {
    if crate::services::is_portable_build()
        || std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("portable.txt").exists()))
            .unwrap_or(false)
    {
        return Err("便携版不支持重置存储路径".into());
    }
    let default_dir = get_default_data_dir()?;
    let current_dir = get_current_storage_dir()?;

    if current_dir == default_dir {
        return Err("当前已在默认存储位置".to_string());
    }

    change_storage_dir_internal(&current_dir, &default_dir, mode)?;

    let mut settings = get_settings();
    settings.use_custom_storage = false;
    settings.custom_storage_path = None;
    update_settings(settings.clone())?;

    let db_path = default_dir.join("quickclipboard.db");
    init_database(db_path.to_str().ok_or("数据库路径无效")?)?;
    let _ = crate::services::database::connection::with_connection(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
    });

    Ok(default_dir)
}

fn change_storage_dir_internal(src_dir: &Path, dst_dir: &Path, mode: &str) -> Result<(), String> {
    let _ = crate::services::database::connection::with_connection(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(FULL); PRAGMA wal_checkpoint(TRUNCATE);")
    });
    let _ = backup_full_zip(src_dir);
    if check_target_has_data(dst_dir)?.has_data {
        let _ = backup_full_zip(dst_dir);
    }

    close_database();

    let src_images = src_dir.join("clipboard_images");
    let dst_images = dst_dir.join("clipboard_images");
    let src_pin_images = src_dir.join("pin_images");
    let dst_pin_images = dst_dir.join("pin_images");
    let src_image_library = src_dir.join("image_library");
    let dst_image_library = dst_dir.join("image_library");
    let src_app_icons = src_dir.join("app_icons");
    let dst_app_icons = dst_dir.join("app_icons");
    let src_db = src_dir.join("quickclipboard.db");
    let dst_db = dst_dir.join("quickclipboard.db");

    match mode {
        "source_only" => {
            if dst_images.exists() {
                fs::remove_dir_all(&dst_images)
                    .map_err(|e| format!("删除目标图片目录失败: {}", e))?;
            }
            if dst_pin_images.exists() {
                fs::remove_dir_all(&dst_pin_images)
                    .map_err(|e| format!("删除目标贴图目录失败: {}", e))?;
            }
            if dst_image_library.exists() {
                fs::remove_dir_all(&dst_image_library)
                    .map_err(|e| format!("删除目标图库目录失败: {}", e))?;
            }
            if dst_app_icons.exists() {
                fs::remove_dir_all(&dst_app_icons)
                    .map_err(|e| format!("删除目标图标目录失败: {}", e))?;
            }
            if dst_db.exists() {
                fs::remove_file(&dst_db).map_err(|e| format!("删除目标数据库失败: {}", e))?;
            }
            if src_images.exists() {
                safe_move_item(&src_images, &dst_images)?;
            }
            if src_pin_images.exists() {
                safe_move_item(&src_pin_images, &dst_pin_images)?;
            }
            if src_image_library.exists() {
                safe_move_item(&src_image_library, &dst_image_library)?;
            }
            if src_app_icons.exists() {
                safe_move_item(&src_app_icons, &dst_app_icons)?;
            }
            if src_db.exists() {
                safe_move_item(&src_db, &dst_db)?;
            }
        }
        "target_only" => {
            if src_images.exists() {
                fs::remove_dir_all(&src_images)
                    .map_err(|e| format!("删除源图片目录失败: {}", e))?;
            }
            if src_pin_images.exists() {
                fs::remove_dir_all(&src_pin_images)
                    .map_err(|e| format!("删除源贴图目录失败: {}", e))?;
            }
            if src_image_library.exists() {
                fs::remove_dir_all(&src_image_library)
                    .map_err(|e| format!("删除源图库目录失败: {}", e))?;
            }
            if src_app_icons.exists() {
                fs::remove_dir_all(&src_app_icons)
                    .map_err(|e| format!("删除源图标目录失败: {}", e))?;
            }
            if src_db.exists() {
                fs::remove_file(&src_db).map_err(|e| format!("删除源数据库失败: {}", e))?;
            }
        }
        "merge" => {
            // 源数据优先：先把目标数据合并到源，再移动源到目标
            if src_images.exists() {
                if !dst_images.exists() {
                    fs::create_dir_all(&dst_images).map_err(|e| e.to_string())?;
                }
                if dst_images.exists() {
                    merge_dir_no_overwrite(&dst_images, &src_images)?;
                }
                if dst_images.exists() {
                    fs::remove_dir_all(&dst_images)
                        .map_err(|e| format!("删除目标图片目录失败: {}", e))?;
                }
                safe_move_item(&src_images, &dst_images)?;
            }
            if src_pin_images.exists() {
                if !dst_pin_images.exists() {
                    fs::create_dir_all(&dst_pin_images).map_err(|e| e.to_string())?;
                }
                if dst_pin_images.exists() {
                    merge_dir_no_overwrite(&dst_pin_images, &src_pin_images)?;
                }
                if dst_pin_images.exists() {
                    fs::remove_dir_all(&dst_pin_images)
                        .map_err(|e| format!("删除目标贴图目录失败: {}", e))?;
                }
                safe_move_item(&src_pin_images, &dst_pin_images)?;
            }
            if src_image_library.exists() {
                if !dst_image_library.exists() {
                    fs::create_dir_all(&dst_image_library).map_err(|e| e.to_string())?;
                }
                if dst_image_library.exists() {
                    merge_dir_no_overwrite(&dst_image_library, &src_image_library)?;
                }
                if dst_image_library.exists() {
                    fs::remove_dir_all(&dst_image_library)
                        .map_err(|e| format!("删除目标图库目录失败: {}", e))?;
                }
                safe_move_item(&src_image_library, &dst_image_library)?;
            }
            if src_app_icons.exists() {
                if !dst_app_icons.exists() {
                    fs::create_dir_all(&dst_app_icons).map_err(|e| e.to_string())?;
                }
                if dst_app_icons.exists() {
                    merge_dir_no_overwrite(&dst_app_icons, &src_app_icons)?;
                }
                if dst_app_icons.exists() {
                    fs::remove_dir_all(&dst_app_icons)
                        .map_err(|e| format!("删除目标图标目录失败: {}", e))?;
                }
                safe_move_item(&src_app_icons, &dst_app_icons)?;
            }
            if src_db.exists() {
                if dst_db.exists() {
                    init_database(src_db.to_str().ok_or("数据库路径无效")?)?;
                    merge_database(&dst_db)?;
                    close_database();
                    fs::remove_file(&dst_db).map_err(|e| format!("删除目标数据库失败: {}", e))?;
                }
                safe_move_item(&src_db, &dst_db)?;
            }
        }
        _ => {
            return Err(format!("不支持的迁移模式: {}", mode));
        }
    }

    for name in ["quickclipboard.db-shm", "quickclipboard.db-wal"] {
        let p = dst_dir.join(name);
        if p.exists() {
            let _ = fs::remove_file(&p);
        }
        let sp = src_dir.join(name);
        if sp.exists() {
            let _ = fs::remove_file(&sp);
        }
    }

    Ok(())
}

pub fn export_data_zip(target_path: PathBuf) -> Result<PathBuf, String> {
    let current_dir = get_current_storage_dir()?;
    let _ = crate::services::database::connection::with_connection(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(FULL); PRAGMA wal_checkpoint(TRUNCATE);")
    });
    close_database();

    let images_dir = current_dir.join("clipboard_images");
    let image_library_dir = current_dir.join("image_library");
    let app_icons_dir = current_dir.join("app_icons");
    let db_files = ["quickclipboard.db"];
    let settings_path = crate::services::settings::storage::SettingsStorage::get_settings_path()?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = fs::File::create(&target_path).map_err(|e| format!("创建导出文件失败: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for name in &db_files {
        let src = current_dir.join(name);
        if src.exists() {
            let mut f = fs::File::open(&src).map_err(|e| format!("读取文件失败: {}", e))?;
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
        }
    }

    fn add_dir_to_zip(
        base: &Path,
        dir: &Path,
        prefix: &str,
        zip: &mut zip::ZipWriter<fs::File>,
        options: zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let rel = path.strip_prefix(base).map_err(|e| e.to_string())?;
            if path.is_dir() {
                add_dir_to_zip(base, &path, prefix, zip, options)?;
            } else {
                let zip_path = Path::new(prefix).join(rel);
                let mut f = fs::File::open(&path).map_err(|e| format!("读取文件失败: {}", e))?;
                let zip_name = zip_path.to_string_lossy();
                zip.start_file(zip_name.as_ref(), options)
                    .map_err(|e| e.to_string())?;
                std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    if images_dir.exists() {
        add_dir_to_zip(
            &images_dir,
            &images_dir,
            "clipboard_images",
            &mut zip,
            options,
        )?;
    }

    if image_library_dir.exists() {
        add_dir_to_zip(
            &image_library_dir,
            &image_library_dir,
            "image_library",
            &mut zip,
            options,
        )?;
    }

    if app_icons_dir.exists() {
        add_dir_to_zip(
            &app_icons_dir,
            &app_icons_dir,
            "app_icons",
            &mut zip,
            options,
        )?;
    }

    if settings_path.exists() {
        let mut f =
            fs::File::open(&settings_path).map_err(|e| format!("读取settings失败: {}", e))?;
        zip.start_file("settings.json", options)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;

    let db_path = current_dir.join("quickclipboard.db");
    if db_path.exists() {
        init_database(db_path.to_str().ok_or("数据库路径无效")?)?;
    }

    Ok(target_path)
}
