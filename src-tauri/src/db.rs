use std::collections::HashMap;
use std::path::Path;
use rusqlite::{params, Connection};
use serde_json;
use serde::Serialize;

use crate::error::AppError;
use crate::models::{DbEntry, FileStats};

/// Папка рабочих пространств перевода (по паре языков) внутри проекта.
fn renforge_dir(project_path: &str) -> std::path::PathBuf {
    Path::new(project_path).join(".renforge")
}

/// Санитизация токена языка под имя файла (буквы/цифры, остальное -> '_').
fn sanitize_token(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() { out.push(c); } else { out.push('_'); }
    }
    if out.is_empty() { out.push('x'); }
    out
}

/// Имя пары source->target (без расширения), напр. "english-russian".
pub fn pair_name(source: &str, target: &str) -> String {
    format!("{}-{}", sanitize_token(source), sanitize_token(target))
}

/// Путь к активной БД: указатель .renforge/active -> .renforge/<pair>.db.
/// Если указателя нет — legacy fallback на project/renforge.db (старые проекты).
fn active_db_path(project_path: &str) -> std::path::PathBuf {
    let dir = renforge_dir(project_path);
    if let Ok(pair) = std::fs::read_to_string(dir.join("active")) {
        let pair = pair.trim();
        if !pair.is_empty() {
            return dir.join(format!("{}.db", pair));
        }
    }
    Path::new(project_path).join("renforge.db")
}

pub fn get_db_conn(project_path: &str) -> Result<Connection, AppError> {
    let db_path = active_db_path(project_path);
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(db_path)?;
    
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;"
    )?;

    // Создаем таблицу с новой колонкой prefix
    conn.execute(
        "CREATE TABLE IF NOT EXISTS translations (
            id TEXT PRIMARY KEY,
            block_type TEXT,
            file_path TEXT,
            line_number INTEGER,
            who TEXT,
            original TEXT,
            translation TEXT,
            status TEXT,
            prefix TEXT,
            prev_original TEXT,
            channel TEXT
        )",
        [],
    )?;

    // Таблица для маппинга персонажей (code → name)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS characters (
            code TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            file_path TEXT,
            line_number INTEGER
        )",
        [],
    )?;

    // Попытка добавить колонку в существующую базу (миграция для старых проектов)
    // Если колонка уже есть, SQLite выдаст ошибку, которую мы просто игнорируем
    let _ = conn.execute("ALTER TABLE translations ADD COLUMN prefix TEXT", []);
    let _ = conn.execute("ALTER TABLE translations ADD COLUMN prev_original TEXT", []);
    // Канал доставки (override): NULL/auto = по block_type, 'say'|'ui'|'both' = принудительно.
    let _ = conn.execute("ALTER TABLE translations ADD COLUMN channel TEXT", []);

    // Индекс по file_path: открытие файла в редакторе (WHERE file_path = ?) и подсчёт
    // статистики (GROUP BY file_path) без него шли полным сканом таблицы — на больших
    // проектах (100k+ строк) это и есть «долгое открытие». Индекс делает выборку точечной.
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_translations_file ON translations(file_path)", []);

    Ok(conn)
}

#[tauri::command]
pub fn search_in_db(project_path: String, query: String) -> Result<Vec<DbEntry>, AppError> {
    let conn = get_db_conn(&project_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, block_type, file_path, line_number, who, original, translation, status, prefix, prev_original 
         FROM translations 
         WHERE original LIKE ?1 OR translation LIKE ?1 LIMIT 100"
    )?;

    let rows = stmt.query_map(params![format!("%{}%", query)], |row| {
        Ok(DbEntry {
            id: row.get(0)?,
            block_type: row.get(1)?,
            file_path: row.get(2)?,
            line_number: row.get(3)?,
            who: row.get(4)?,
            original: row.get(5)?,
            translation: row.get(6)?,
            status: row.get(7)?,
            prefix: row.get(8)?,
            prev_original: row.get(9)?,
            channel: None,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

#[tauri::command]
pub fn get_translation_stats(project_path: String) -> Result<HashMap<String, FileStats>, AppError> {
    let conn = get_db_conn(&project_path)?;
    let mut stmt = conn.prepare(
        "SELECT file_path, COUNT(id), SUM(CASE WHEN status = 'translated' THEN 1 ELSE 0 END), SUM(CASE WHEN status = 'outdated' THEN 1 ELSE 0 END) 
         FROM translations GROUP BY file_path"
    )?;

    let rows = stmt.query_map([], |row| {
        let path: String = row.get(0)?;
        let total: i32 = row.get(1)?;
        let translated: i32 = row.get(2).unwrap_or(0);
        let outdated: i32 = row.get(3).unwrap_or(0);
        Ok((path, FileStats { total, translated, outdated }))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (path, stats) = row?;
        map.insert(path, stats);
    }
    Ok(map)
}

#[tauri::command]
pub fn upsert_translations_batch(project_path: String, entries: Vec<DbEntry>) -> Result<(), AppError> {
    let mut conn = get_db_conn(&project_path)?;
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO translations (id, block_type, file_path, line_number, who, original, translation, status, prefix, prev_original, channel)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
        )?;

        // Дедупликация: собираем пары (original → translation) для распространения
        let mut dedup_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for entry in &entries {
            stmt.execute(params![
                entry.id, entry.block_type, entry.file_path, entry.line_number, 
                entry.who, entry.original, entry.translation, entry.status, entry.prefix, entry.prev_original, entry.channel
            ])?;
            
            // Если строка переведена, запоминаем для дедупликации
            if entry.status == "translated" && !entry.translation.is_empty() {
                dedup_map.insert(entry.original.clone(), entry.translation.clone());
            }
        }

        // Применяем переводы ко всем дубликатам в БД (строки с тем же original, но в других файлах/местах)
        if !dedup_map.is_empty() {
            let mut dedup_stmt = tx.prepare(
                "UPDATE translations SET translation = ?1, status = 'translated' 
                 WHERE original = ?2 AND (translation = '' OR translation IS NULL)"
            )?;
            
            for (original, translation) in &dedup_map {
                let _ = dedup_stmt.execute(params![translation, original]);
            }
        }
    }
    tx.commit()?;
    // Помечаем активную пару как «изменённую после сборки»: если её мод уже собран
    // (лежит в game/), экспорт/UI предупредят, что собранный мод устарел относительно БД.
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)", []);
    let _ = conn.execute("INSERT OR REPLACE INTO project_meta (key, value) VALUES ('built_dirty', '1')", []);
    Ok(())
}

/// Удаление строк по списку id (для ручных строк: удалить/переименовать-через-пересоздание).
#[tauri::command]
pub fn delete_translations(project_path: String, ids: Vec<String>) -> Result<(), AppError> {
    let mut conn = get_db_conn(&project_path)?;
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("DELETE FROM translations WHERE id = ?1")?;
        for id in &ids {
            let _ = stmt.execute(params![id]);
        }
    }
    tx.commit()?;
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)", []);
    let _ = conn.execute("INSERT OR REPLACE INTO project_meta (key, value) VALUES ('built_dirty', '1')", []);
    Ok(())
}

#[tauri::command]
pub fn get_translations_for_file(project_path: String, file_path: String) -> Result<Vec<DbEntry>, AppError> {
    let conn = get_db_conn(&project_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, block_type, file_path, line_number, who, original, translation, status, prefix, prev_original, channel 
         FROM translations WHERE file_path = ?1 ORDER BY line_number ASC"
    )?;

    let rows = stmt.query_map(rusqlite::params![file_path], |row| {
        Ok(DbEntry {
            id: row.get(0)?,
            block_type: row.get(1)?,
            file_path: row.get(2)?,
            line_number: row.get(3)?,
            who: row.get(4)?,
            original: row.get(5)?,
            translation: row.get(6)?,
            status: row.get(7)?,
            prefix: row.get(8)?,
            prev_original: row.get(9)?,
            channel: row.get(10)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}


#[tauri::command]
pub fn get_project_languages(project_path: String) -> Result<Vec<String>, AppError> {
    let conn = get_db_conn(&project_path)?;
    
    // Создаём таблицу если не существует (для случая когда БД ещё пуста)
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)",
        []
    );
    
    let result: Result<String, _> = conn.query_row(
        "SELECT value FROM project_meta WHERE key = 'available_languages'",
        [],
        |row| row.get(0)
    );
    
    match result {
        Ok(json_str) => {
            let langs: Vec<String> = serde_json::from_str(&json_str).unwrap_or_default();
            Ok(langs)
        }
        Err(_) => Ok(Vec::new())
    }
}


#[tauri::command]
pub fn get_project_meta(project_path: String, key: String) -> Result<Option<String>, AppError> {
    let conn = get_db_conn(&project_path)?;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)",
        []
    );
    let result = conn.query_row(
        "SELECT value FROM project_meta WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0)
    );
    match result {
        Ok(val) => Ok(Some(val)),
        Err(_) => Ok(None)
    }
}

#[derive(Serialize)]
pub struct PairInfo {
    pub pair: String,        // имя файла без .db ("english-russian") или "" для legacy
    pub source: String,      // язык-источник (из project_meta)
    pub target: String,      // язык перевода (из project_meta)
    pub total: i32,
    pub translated: i32,
    pub is_active: bool,
    pub is_legacy: bool,     // true для старого project/renforge.db
    pub is_built: bool,      // мод этой пары сейчас материализован в game/ (можно экспортировать)
    pub is_dirty: bool,      // БД изменена после последней сборки мода (собранный мод устарел)
}

fn read_pair_info(db_path: &Path, pair: &str, is_legacy: bool, active_pair: &str, built_pair: &Option<String>) -> Option<PairInfo> {
    let conn = Connection::open(db_path).ok()?;
    let meta = |key: &str| -> String {
        conn.query_row(
            "SELECT value FROM project_meta WHERE key = ?1",
            params![key], |r| r.get::<_, String>(0)
        ).unwrap_or_default()
    };
    let source = meta("source_language");
    let target = meta("target_language");
    let (total, translated): (i32, i32) = conn.query_row(
        "SELECT COUNT(id), SUM(CASE WHEN status='translated' THEN 1 ELSE 0 END) FROM translations",
        [], |r| Ok((r.get(0)?, r.get::<_, Option<i32>>(1)?.unwrap_or(0)))
    ).unwrap_or((0, 0));
    let is_active = if is_legacy { active_pair.is_empty() } else { pair == active_pair };
    // is_built: указатель .renforge/built хранит имя пары, чей мод сейчас в game/.
    // None = ничего не собрано. Для legacy указатель пустой ("").
    let is_built = match built_pair {
        Some(b) => if is_legacy { b.is_empty() } else { pair == b },
        None => false,
    };
    let is_dirty = meta("built_dirty") == "1";
    Some(PairInfo {
        pair: pair.to_string(),
        source, target, total, translated, is_active, is_legacy, is_built, is_dirty,
    })
}

/// Записывает указатель .renforge/built = текущая активная пара (или "" для legacy).
/// Вызывается после успешной сборки мода — этим помечаем, чей мод лежит в game/.
/// Заодно сбрасываем флаг built_dirty: собранный мод теперь соответствует БД.
pub fn mark_pair_built(project_path: &str) {
    let dir = renforge_dir(project_path);
    let _ = std::fs::create_dir_all(&dir);
    let active = std::fs::read_to_string(dir.join("active")).unwrap_or_default().trim().to_string();
    let _ = std::fs::write(dir.join("built"), &active);
    if let Ok(conn) = get_db_conn(project_path) {
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)", []);
        let _ = conn.execute("INSERT OR REPLACE INTO project_meta (key, value) VALUES ('built_dirty', '0')", []);
    }
}

/// Список рабочих пространств перевода (пар языков) проекта + legacy renforge.db.
#[tauri::command]
pub fn list_translation_pairs(project_path: String) -> Result<Vec<PairInfo>, String> {
    let dir = renforge_dir(&project_path);
    let active = std::fs::read_to_string(dir.join("active")).unwrap_or_default().trim().to_string();
    let built: Option<String> = std::fs::read_to_string(dir.join("built")).ok().map(|s| s.trim().to_string());
    let mut out: Vec<PairInfo> = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?.path();
            if p.extension().and_then(|s| s.to_str()) == Some("db") {
                let pair = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Some(info) = read_pair_info(&p, &pair, false, &active, &built) {
                    if info.total > 0 { out.push(info); }  // пустые рабочие пространства не показываем
                }
            }
        }
    }
    // legacy renforge.db (старые проекты) — только если в нём реально что-то есть
    let legacy = Path::new(&project_path).join("renforge.db");
    if legacy.exists() {
        if let Some(info) = read_pair_info(&legacy, "", true, &active, &built) {
            if info.total > 0 { out.push(info); }
        }
    }
    // активные сверху, затем по числу строк
    out.sort_by(|a, b| b.is_active.cmp(&a.is_active).then(b.total.cmp(&a.total)));
    Ok(out)
}

/// Переключить активное рабочее пространство на пару source->target.
/// Создаёт .renforge/active с именем пары. Сама БД создастся при первом get_db_conn.
#[tauri::command]
pub fn set_active_pair(project_path: String, source: String, target: String) -> Result<String, String> {
    let dir = renforge_dir(&project_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let pair = pair_name(&source, &target);
    std::fs::write(dir.join("active"), &pair).map_err(|e| e.to_string())?;
    Ok(pair)
}

/// Переключиться на legacy renforge.db (убрать указатель активной пары).
#[tauri::command]
pub fn use_legacy_db(project_path: String) -> Result<(), String> {
    let active = renforge_dir(&project_path).join("active");
    if active.exists() {
        std::fs::remove_file(&active).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Удалить рабочее пространство (БД пары). Если удаляем активное — сбрасываем указатель.
#[tauri::command]
pub fn delete_translation_pair(project_path: String, pair: String) -> Result<(), String> {
    let dir = renforge_dir(&project_path);
    let db = dir.join(format!("{}.db", pair));
    if db.exists() {
        std::fs::remove_file(&db).map_err(|e| e.to_string())?;
    }
    // подчистим WAL/SHM
    for ext in ["db-wal", "db-shm"] {
        let p = dir.join(format!("{}.{}", pair, ext));
        if p.exists() { let _ = std::fs::remove_file(&p); }
    }
    let active = dir.join("active");
    if std::fs::read_to_string(&active).unwrap_or_default().trim() == pair {
        let _ = std::fs::remove_file(&active);
    }
    // если удаляем пару, чей мод материализован — сбрасываем указатель built
    let built = dir.join("built");
    if std::fs::read_to_string(&built).unwrap_or_default().trim() == pair {
        let _ = std::fs::remove_file(&built);
    }
    Ok(())
}
