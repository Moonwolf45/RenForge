// Глобальная Translation Memory (TM): кросс-проектное хранилище переводов пользователя.
// Копится автоматически при сохранении, переиспользуется в новых проектах (точные совпадения),
// модерируется через собственный редактор (поиск/правка/удаление). База — tm.db в app-data.
use rusqlite::Connection;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

#[derive(serde::Serialize)]
pub struct TmEntry {
    pub target_lang: String,
    pub original: String,
    pub translation: String,
    pub source_lang: String,
    pub hits: i64,
}

#[derive(serde::Serialize)]
pub struct TmListResult {
    pub entries: Vec<TmEntry>,
    pub total: i64,
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn tm_conn(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let conn = Connection::open(dir.join("tm.db")).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tm (
            target_lang TEXT NOT NULL,
            original TEXT NOT NULL,
            translation TEXT NOT NULL,
            source_lang TEXT,
            hits INTEGER DEFAULT 1,
            updated_at INTEGER,
            PRIMARY KEY (target_lang, original)
        );
        CREATE INDEX IF NOT EXISTS idx_tm_target ON tm(target_lang);"
    ).map_err(|e| e.to_string())?;
    Ok(conn)
}

/// Авто-наполнение: заливает все translated-строки активной пары проекта в TM.
/// Вызывается фронтом после сохранения. Возвращает число занесённых пар.
#[tauri::command]
pub fn tm_contribute(app: tauri::AppHandle, project_path: String) -> Result<i64, String> {
    let pconn = crate::db::get_db_conn(&project_path).map_err(|e| e.to_string())?;
    let meta = |k: &str| -> String {
        pconn.query_row("SELECT value FROM project_meta WHERE key=?1", rusqlite::params![k], |r| r.get::<_, String>(0)).unwrap_or_default()
    };
    let target = meta("target_language");
    let source = meta("source_language");
    if target.trim().is_empty() { return Ok(0); }

    let pairs: Vec<(String, String)> = {
        let mut stmt = pconn.prepare(
            "SELECT original, translation FROM translations \
             WHERE status='translated' AND translation IS NOT NULL AND translation != ''"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|x| x.ok()).collect()
    };

    let mut conn = tm_conn(&app)?;
    let now = now_secs();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut n = 0i64;
    {
        let mut up = tx.prepare(
            "INSERT INTO tm (target_lang, original, translation, source_lang, hits, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 1, ?5) \
             ON CONFLICT(target_lang, original) DO UPDATE SET \
                translation=excluded.translation, source_lang=excluded.source_lang, \
                hits=hits+1, updated_at=excluded.updated_at"
        ).map_err(|e| e.to_string())?;
        for (o, t) in &pairs {
            if o.trim().is_empty() { continue; }
            let _ = up.execute(rusqlite::params![target, o, t, source, now]);
            n += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(n)
}

/// Safe-заливка: для активной пары подставляет в НЕпереведённые строки точные совпадения из TM.
/// Помечает как «требует проверки» (status='outdated' + prev_original=original — переиспользует
/// очередь проверки без ложного диффа «Было»). Возвращает число заполненных.
#[tauri::command]
pub fn tm_fill(app: tauri::AppHandle, project_path: String) -> Result<i64, String> {
    let pconn = crate::db::get_db_conn(&project_path).map_err(|e| e.to_string())?;
    let target: String = pconn.query_row(
        "SELECT value FROM project_meta WHERE key='target_language'", [], |r| r.get::<_, String>(0)
    ).unwrap_or_default();
    if target.trim().is_empty() { return Ok(0); }

    // Загружаем TM для целевого языка в память
    let tmconn = tm_conn(&app)?;
    let mut tm: HashMap<String, String> = HashMap::new();
    {
        let mut stmt = tmconn.prepare("SELECT original, translation FROM tm WHERE target_lang=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![target], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() { tm.insert(row.0, row.1); }
    }
    if tm.is_empty() { return Ok(0); }

    // Кандидаты: непереведённые строки, чей оригинал есть в TM
    let cands: Vec<(String, String, String)> = {
        let mut stmt = pconn.prepare(
            "SELECT id, original FROM translations WHERE (translation IS NULL OR translation='')"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|x| x.ok())
            .filter_map(|(id, orig)| tm.get(&orig).map(|tr| (id, orig, tr.clone())))
            .collect()
    };

    let mut n = 0i64;
    {
        let mut upd = pconn.prepare(
            "UPDATE translations SET translation=?1, status='outdated', prev_original=?2 WHERE id=?3"
        ).map_err(|e| e.to_string())?;
        for (id, orig, tr) in &cands {
            if upd.execute(rusqlite::params![tr, orig, id]).unwrap_or(0) > 0 { n += 1; }
        }
    }
    Ok(n)
}

/// Список/поиск записей TM для редактора (пагинация). query пустой = все.
#[tauri::command]
pub fn tm_list(app: tauri::AppHandle, query: String, limit: i64, offset: i64) -> Result<TmListResult, String> {
    let conn = tm_conn(&app)?;
    let q = query.trim();
    let like = format!("%{}%", q);
    let total: i64 = if q.is_empty() {
        conn.query_row("SELECT COUNT(*) FROM tm", [], |r| r.get(0)).unwrap_or(0)
    } else {
        conn.query_row("SELECT COUNT(*) FROM tm WHERE original LIKE ?1 OR translation LIKE ?1",
            rusqlite::params![like], |r| r.get(0)).unwrap_or(0)
    };
    let mut entries = Vec::new();
    if q.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT target_lang, original, translation, COALESCE(source_lang,''), hits \
             FROM tm ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![limit, offset], |r| Ok(TmEntry {
            target_lang: r.get(0)?, original: r.get(1)?, translation: r.get(2)?,
            source_lang: r.get(3)?, hits: r.get(4)?,
        })).map_err(|e| e.to_string())?;
        for e in rows.flatten() { entries.push(e); }
    } else {
        let mut stmt = conn.prepare(
            "SELECT target_lang, original, translation, COALESCE(source_lang,''), hits \
             FROM tm WHERE original LIKE ?1 OR translation LIKE ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![like, limit, offset], |r| Ok(TmEntry {
            target_lang: r.get(0)?, original: r.get(1)?, translation: r.get(2)?,
            source_lang: r.get(3)?, hits: r.get(4)?,
        })).map_err(|e| e.to_string())?;
        for e in rows.flatten() { entries.push(e); }
    }
    Ok(TmListResult { entries, total })
}

/// Добавить/изменить запись TM вручную (редактор).
#[tauri::command]
pub fn tm_upsert(app: tauri::AppHandle, target_lang: String, original: String, translation: String, source_lang: String) -> Result<(), String> {
    if target_lang.trim().is_empty() || original.trim().is_empty() {
        return Err("target_lang и original обязательны".to_string());
    }
    let conn = tm_conn(&app)?;
    conn.execute(
        "INSERT INTO tm (target_lang, original, translation, source_lang, hits, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 1, ?5) \
         ON CONFLICT(target_lang, original) DO UPDATE SET \
            translation=excluded.translation, source_lang=excluded.source_lang, updated_at=excluded.updated_at",
        rusqlite::params![target_lang, original, translation, source_lang, now_secs()]
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Удалить запись TM.
#[tauri::command]
pub fn tm_delete(app: tauri::AppHandle, target_lang: String, original: String) -> Result<(), String> {
    let conn = tm_conn(&app)?;
    conn.execute("DELETE FROM tm WHERE target_lang=?1 AND original=?2",
        rusqlite::params![target_lang, original]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Размер TM (число записей).
#[tauri::command]
pub fn tm_count(app: tauri::AppHandle) -> Result<i64, String> {
    let conn = tm_conn(&app)?;
    Ok(conn.query_row("SELECT COUNT(*) FROM tm", [], |r| r.get(0)).unwrap_or(0))
}

/// Полная очистка TM.
#[tauri::command]
pub fn tm_clear(app: tauri::AppHandle) -> Result<(), String> {
    let conn = tm_conn(&app)?;
    conn.execute("DELETE FROM tm", []).map_err(|e| e.to_string())?;
    Ok(())
}
