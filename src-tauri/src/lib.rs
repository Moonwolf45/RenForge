pub mod db;
pub mod error;
pub mod models;
pub mod tm;

use error::AppError;
use models::{AudioEntry, ImageEntry, ProjectFiles};

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri_plugin_shell::ShellExt;
use walkdir::WalkDir;

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let p_lower = path.to_lowercase();
    if !p_lower.ends_with(".csv") && !p_lower.ends_with(".json")
        && !p_lower.ends_with(".po") && !p_lower.ends_with(".pot") {
        return Err("Access denied: File must have a .csv, .json, .po or .pot extension".to_string());
    }
    std::fs::read_to_string(&path).map_err(|e| format!("Ошибка чтения: {}", e))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    let p_lower = path.to_lowercase();
    if !p_lower.ends_with(".csv") && !p_lower.ends_with(".json")
        && !p_lower.ends_with(".po") && !p_lower.ends_with(".pot") {
        return Err("Access denied: File must have a .csv, .json, .po or .pot extension".to_string());
    }
    std::fs::write(&path, content).map_err(|e| format!("Ошибка сохранения: {}", e))
}

#[tauri::command]
async fn scan_project(path: String, target_lang: String) -> Result<ProjectFiles, AppError> {
    tokio::task::spawn_blocking(move || {
        let mut rpa_files = Vec::new();
        let mut rpyc_files = Vec::new();
        let mut rpy_files = Vec::new();
        let mut tl_files = Vec::new();

        let game_dir = Path::new(&path).join("game");
        let tl_dir_str = format!("/tl/{}/", target_lang);
        let tl_dir_str_win = format!("\\tl\\{}\\", target_lang);
        
        if game_dir.exists() {
            for entry in walkdir::WalkDir::new(&game_dir).into_iter().filter_map(|e| e.ok()) {
                let path_str = entry.path().display().to_string();
                let is_tl = path_str.contains(&tl_dir_str) || path_str.contains(&tl_dir_str_win);

                if path_str.ends_with(".rpa") { 
                    rpa_files.push(path_str); 
                }
                else if path_str.ends_with(".rpyc") && !is_tl { 
                    rpyc_files.push(path_str); 
                }
                else if path_str.ends_with(".rpy") { 
                    if is_tl {
                        tl_files.push(path_str);
                    } else {
                        rpy_files.push(path_str);
                    }
                }
            }
        }

        Ok(ProjectFiles { rpa_files, rpyc_files, rpy_files, tl_files })
    })
    .await
    .map_err(|e| AppError::Custom(format!("Сбой фонового потока: {}", e)))?
}

#[tauri::command]
async fn extract_and_ingest_project(app: tauri::AppHandle, project_path: String, source_lang: Option<String>, target_lang: Option<String>) -> Result<crate::models::ExtractResult, String> {
    let game_dir = Path::new(&project_path).join("game");
    let out_json = Path::new(&project_path).join("renforge_ast.json");

    if !game_dir.exists() {
        return Err("game_dir_missing".to_string());
    }

    let lang = source_lang.unwrap_or_else(|| "auto".to_string());
    let target = target_lang.unwrap_or_else(|| "russian".to_string());

    let sidecar = app.shell().sidecar("rpyc_extractor").map_err(|e| e.to_string())?;
    
    let output = sidecar
        .arg("--dir").arg(&game_dir)
        .arg("--out").arg(&out_json)
        .arg("--source-lang").arg(&lang)
        .output()
        .await
        .map_err(|e| format!("extractor_spawn_failed: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("extractor_error:\n{}", err_msg));
    }

    // Разрешаем source (auto -> конкретный) из вывода экстрактора и активируем
    // рабочее пространство пары source->target ДО ingest, чтобы данные легли в нужную БД.
    // Заодно забираем skipped_files (roadmap 1.3) — один и тот же разбор JSON на оба поля.
    let parsed_extracted = std::fs::read_to_string(&out_json).ok()
        .and_then(|c| serde_json::from_str::<crate::models::ExtractedData>(&c).ok());
    let resolved_source = parsed_extracted.as_ref()
        .and_then(|d| d.source_language.clone())
        .unwrap_or_else(|| "original".to_string());
    let skipped_files = parsed_extracted.map(|d| d.skipped_files).unwrap_or_default();
    crate::db::set_active_pair(project_path.clone(), resolved_source, target.clone())?;

    let _ = ingest_extracted_json(&project_path, &out_json)?;

    // Сохраняем язык перевода в мету активной БД (для виджета пар) и возвращаем число
    // извлечённых строк — фронт сам сформирует локализованное сообщение.
    let mut total: i64 = 0;
    if let Ok(conn) = crate::db::get_db_conn(&project_path) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO project_meta (key, value) VALUES ('target_language', ?1)",
            rusqlite::params![target]
        );
        total = conn.query_row("SELECT COUNT(*) FROM translations", [], |r| r.get::<_, i64>(0)).unwrap_or(0);
    }
    Ok(crate::models::ExtractResult { total, skipped_files })
}

/// Быстрое определение языков-источников игры (папки tl/<lang>/ + суффиксы _XX в .rpyc)
/// БЕЗ полного извлечения. Нужно, чтобы UI-селектор «Переводить с» был доступен ДО extract
/// (убирает «курицу и яйцо»: раньше языки появлялись только после первого извлечения).
#[tauri::command]
async fn discover_source_languages(app: tauri::AppHandle, project_path: String) -> Result<Vec<String>, String> {
    let game_dir = Path::new(&project_path).join("game");
    if !game_dir.exists() {
        return Ok(vec![]);
    }
    let sidecar = app.shell().sidecar("rpyc_extractor").map_err(|e| e.to_string())?;
    let output = sidecar
        .arg("--dir").arg(&game_dir)
        .arg("--list-languages")
        .output()
        .await
        .map_err(|e| format!("Не удалось запустить экстрактор: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.trim().strip_prefix("RENFORGE_LANGS:") {
            let langs: Vec<String> = serde_json::from_str(rest.trim()).unwrap_or_default();
            return Ok(langs);
        }
    }
    Ok(vec![])
}


/// Загружает JSON от экстрактора в БД проекта: фильтрация мусора, таблица characters,
/// движковые строки, метаданные. Вынесено из команды, чтобы переиспользовать в
/// headless-CLI и тестах без tauri::AppHandle (полная симуляция продуктового пути).
pub fn ingest_extracted_json(project_path: &str, out_json: &Path) -> Result<String, String> {
    let json_content = std::fs::read_to_string(out_json)
        .map_err(|e| format!("Ошибка чтения JSON: {}", e))?;

    let extracted: crate::models::ExtractedData = serde_json::from_str(&json_content)
        .map_err(|e| format!("Ошибка парсинга JSON: {}", e))?;

    let mut conn = crate::db::get_db_conn(project_path).map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let mut seen_ids = std::collections::HashSet::new();
    let mut skipped_garbage = 0u32;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO translations (id, block_type, file_path, line_number, who, original, translation, status, prefix, source, alt_texts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET 
                block_type = excluded.block_type,
                file_path = excluded.file_path,
                line_number = excluded.line_number,
                who = excluded.who,
                original = excluded.original,
                prefix = excluded.prefix,
                source = excluded.source,
                alt_texts = excluded.alt_texts"
        ).map_err(|e| e.to_string())?;

        // Отдельная таблица для Character mapping (define строки)
        tx.execute(
            "CREATE TABLE IF NOT EXISTS characters (
                code TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                file_path TEXT,
                line_number INTEGER
            )", []
        ).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM characters", []).map_err(|e| e.to_string())?;

        let mut char_stmt = tx.prepare(
            "INSERT OR REPLACE INTO characters (code, name, file_path, line_number)
             VALUES (?1, ?2, ?3, ?4)"
        ).map_err(|e| e.to_string())?;

        for string_data in extracted.strings {
            let what_trimmed = string_data.what.trim();
            
            // --- ФИЛЬТРАЦИЯ МУСОРА ---
            // Пустые строки
            if what_trimmed.is_empty() {
                skipped_garbage += 1;
                continue;
            }
            // Чистые переменные: [config.name!t], [message], [tooltip] и т.д.
            if what_trimmed.starts_with('[') && what_trimmed.ends_with(']') 
               && !what_trimmed.contains(' ') && !what_trimmed.contains('"') {
                skipped_garbage += 1;
                continue;
            }
            // Одиночные символы (кроме осмысленной пунктуации)
            if what_trimmed.chars().count() == 1 {
                let ch = what_trimmed.chars().next().unwrap();
                if !ch.is_alphabetic() || "HSV".contains(ch) {
                    skipped_garbage += 1;
                    continue;
                }
            }
            // Чистые числа
            if what_trimmed.chars().all(|c| c.is_ascii_digit()) && what_trimmed.len() > 1 {
                skipped_garbage += 1;
                continue;
            }

            // --- CHARACTER DEFINITIONS → отдельная таблица + основная ---
            if string_data.block_type == "python" {
                let who = string_data.who.as_deref().unwrap_or("");
                if who.starts_with("[DEFINE:") {
                    // Извлекаем код переменной из "[DEFINE: varname]"
                    let code = who.trim_start_matches("[DEFINE:").trim_end_matches(']').trim();
                    if !code.is_empty() && !what_trimmed.is_empty() {
                        let _ = char_stmt.execute(rusqlite::params![
                            code,
                            what_trimmed,
                            string_data.file,
                            string_data.line
                        ]);
                    }
                    // НЕ пропускаем — записываем также в translations для перевода
                }
            }

            // --- ЗАПИСЬ В ОСНОВНУЮ ТАБЛИЦУ ---
            let mut final_id = string_data.id.clone();
            let mut counter = 1;
            while seen_ids.contains(&final_id) {
                final_id = format!("{}_{}", string_data.id, counter);
                counter += 1;
            }
            seen_ids.insert(final_id.clone());

            // alt-тексты (multi-key) → JSON-массив в TEXT-колонку; пусто → NULL.
            let alt_json: Option<String> = if string_data.alt_texts.is_empty() {
                None
            } else {
                serde_json::to_string(&string_data.alt_texts).ok()
            };
            stmt.execute(rusqlite::params![
                final_id,
                string_data.block_type,
                string_data.file,
                string_data.line,
                string_data.who,
                string_data.what,
                "", 
                "untranslated",
                string_data.prefix,
                string_data.source,
                alt_json
            ]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    let _ = std::fs::remove_file(&out_json);

    // Движковые строки Ren'Py (renpy/common): экстрактор берёт их из renpy/common/*.rpy
    // (см. main.py extract_engine_common). Здесь — supplement: стражглеры, которые регексп
    // не поймал, + фоллбэк для игр без renpy/common-исходников. Вставляем по тексту с дедупом
    // (WHERE NOT EXISTS), чтобы не задвоить уже извлечённое.
    {
        let engine_strings = [
            "Are you sure you want to quit?",
            "Are you sure you want to return to the main menu?\nThis will lose unsaved progress.",
            "Are you sure you want to overwrite your save?",
            "Are you sure you want to delete this save?",
            "Loading will lose unsaved progress.\nAre you sure you want to do this?",
            "Are you sure you want to continue where you left off?",
            "Skip unseen dialogue to the next choice?",
            // стражглеры (не ловятся регекспом из common-исходника на части движков)
            "Skip Mode",
            "Empty Slot.",
            "Previous",
            "Next",
        ];
        if let Ok(mut stmt) = conn.prepare(
            "INSERT INTO translations \
             (id, block_type, file_path, line_number, who, original, translation, status, prefix, source) \
             SELECT ?1, 'ui', 'engine (renpy common)', 0, '[ENGINE]', ?2, '', 'untranslated', NULL, 'regex' \
             WHERE NOT EXISTS (SELECT 1 FROM translations WHERE original = ?2)"
        ) {
            for (i, s) in engine_strings.iter().enumerate() {
                let _ = stmt.execute(rusqlite::params![format!("engine_sup_{:04}", i), s]);
            }
        }
    }


    if let Some(ref langs) = extracted.available_languages {
        let langs_json = serde_json::to_string(langs).unwrap_or_default();
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)",
            []
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO project_meta (key, value) VALUES ('available_languages', ?1)",
            rusqlite::params![langs_json]
        );
        if let Some(ref src_lang) = extracted.source_language {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO project_meta (key, value) VALUES ('source_language', ?1)",
                rusqlite::params![src_lang]
            );
        }
    }
    
    // Сохраняем флаг legacy-формата
    let is_legacy = extracted.is_legacy_format.unwrap_or(false);
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)",
        []
    );
    let _ = conn.execute(
        "INSERT OR REPLACE INTO project_meta (key, value) VALUES ('is_legacy', ?1)",
        rusqlite::params![if is_legacy { "true" } else { "false" }]
    );

    // Инфо об игре (имя/версия/движок) — для шапки дашборда
    for (key, val) in [
        ("game_name", &extracted.game_name),
        ("game_version", &extracted.game_version),
        ("engine_version", &extracted.engine_version),
    ] {
        if let Some(v) = val {
            if !v.trim().is_empty() {
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO project_meta (key, value) VALUES (?1, ?2)",
                    rusqlite::params![key, v]
                );
            }
        }
    }

    let mut final_msg = format!("Извлечено строк: {} (отфильтровано мусора: {})", 
        seen_ids.len(), skipped_garbage);
    if extracted.is_legacy_format.unwrap_or(false) {
        final_msg.push_str(" \nВнимание: обнаружен старый движок Ren'Py! Внедрение переводов для этой версии может потребовать ручной адаптации.");
    }

    Ok(final_msg)
}

/// Переносит готовый перевод из старой версии игры (old_project_path) в новую
/// (new_project_path). Точное совпадение id → перенос как 'translated';
/// изменённый текст (fuzzy по тому же файлу) → перенос как 'outdated' + prev_original.
pub fn migrate_translations_core(new_project_path: String, old_project_path: String)
    -> Result<crate::models::MigrationReport, String>
{
    use std::collections::{HashMap, HashSet};

    let old_db = Path::new(&old_project_path).join("renforge.db");
    if !old_db.exists() {
        return Err("В выбранной папке нет renforge.db — похоже, она не переводилась в RenForge.".to_string());
    }

    let old_conn = rusqlite::Connection::open(&old_db).map_err(|e| e.to_string())?;

    // Загружаем строки старой версии:
    //  - old_all_ids: все id (для отличия «новых» от «были и раньше»)
    //  - old_trans_by_id: id -> перевод (только непустые) для точного переноса
    //  - old_file_trans: переведённые строки по файлам (для fuzzy-переноса)
    //  - old_file_origs: ВСЕ оригиналы по файлам (чтобы понять, новая ли строка)
    let mut old_all_ids: HashSet<String> = HashSet::new();
    let mut old_trans_by_id: HashMap<String, String> = HashMap::new();
    let mut old_file_trans: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();
    let mut old_file_origs: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = old_conn.prepare(
            "SELECT id, file_path, original, translation FROM translations"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((
            r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?.unwrap_or_default()
        ))).map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            let (id, file, orig, trans) = row;
            old_all_ids.insert(id.clone());
            old_file_origs.entry(file.clone()).or_default().push(orig.clone());
            if !trans.is_empty() {
                old_trans_by_id.insert(id, trans.clone());
                old_file_trans.entry(file).or_default().push((orig, trans, false));
            }
        }
    }
    let total_old_translated = old_trans_by_id.len() as u32;

    // Новые строки без перевода
    let mut new_conn = crate::db::get_db_conn(&new_project_path).map_err(|e| e.to_string())?;
    let mut new_rows: Vec<(String, String, String)> = Vec::new(); // (id, file, original)
    {
        let mut stmt = new_conn.prepare(
            "SELECT id, file_path, original FROM translations WHERE translation IS NULL OR translation = ''"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((
            r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?
        ))).map_err(|e| e.to_string())?;
        for row in rows.flatten() { new_rows.push(row); }
    }

    let mut report = crate::models::MigrationReport::default();
    // (id, translation, status, prev_original)
    let mut updates: Vec<(String, String, String, Option<String>)> = Vec::new();
    const FUZZY_THRESHOLD: f64 = 0.7;

    for (id, file, original) in &new_rows {
        // 1) Точное совпадение id с переведённой строкой (текст не менялся)
        if let Some(trans) = old_trans_by_id.get(id) {
            updates.push((id.clone(), trans.clone(), "translated".to_string(), None));
            report.carried_exact += 1;
            continue;
        }
        // 2) id существовал в старой версии (был непереведён) — не новая строка
        if old_all_ids.contains(id) {
            report.still_untranslated += 1;
            continue;
        }
        // 3) id новый → текст изменился ИЛИ строка действительно новая.
        //    Сначала пробуем fuzzy-перенос с переведённой строкой того же файла.
        let mut handled = false;
        if let Some(cands) = old_file_trans.get_mut(file) {
            let mut best_i: Option<usize> = None;
            let mut best_score = FUZZY_THRESHOLD;
            for (i, (oorig, _otrans, used)) in cands.iter().enumerate() {
                if *used { continue; }
                let score = strsim::normalized_levenshtein(original, oorig);
                if score > best_score { best_score = score; best_i = Some(i); }
            }
            if let Some(i) = best_i {
                let (oorig, otrans, used) = &mut cands[i];
                *used = true;
                updates.push((id.clone(), otrans.clone(), "outdated".to_string(), Some(oorig.clone())));
                report.carried_fuzzy += 1;
                handled = true;
            }
        }
        if handled { continue; }
        // 4) Перевода рядом нет. Похожа ли строка на какой-либо старый оригинал (пусть и без перевода)?
        //    Если да — это изменённая строка, что и раньше была без перевода (не новая).
        let changed_known = old_file_origs.get(file)
            .map(|v| v.iter().any(|o| strsim::normalized_levenshtein(original, o) > FUZZY_THRESHOLD))
            .unwrap_or(false);
        if changed_known {
            report.still_untranslated += 1;
        } else {
            report.new_strings += 1;
        }
    }

    report.old_unused = total_old_translated.saturating_sub(report.carried_exact + report.carried_fuzzy);

    // Применяем переносы
    let tx = new_conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut stmt = tx.prepare(
            "UPDATE translations SET translation = ?1, status = ?2, prev_original = ?3 WHERE id = ?4"
        ).map_err(|e| e.to_string())?;
        for (id, trans, status, prev) in &updates {
            stmt.execute(rusqlite::params![trans, status, prev, id]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    Ok(report)
}

#[tauri::command]
fn migrate_translations(new_project_path: String, old_project_path: String)
    -> Result<crate::models::MigrationReport, String>
{
    migrate_translations_core(new_project_path, old_project_path)
}

#[tauri::command]
fn get_character_mapping(project_path: String) -> Result<HashMap<String, String>, String> {
    let mut mapping = HashMap::new();
    
    // Сначала пробуем получить маппинг из БД (заполняется экстрактором)
    if let Ok(conn) = crate::db::get_db_conn(&project_path) {
        let result: Result<Vec<(String, String)>, _> = (|| {
            let mut stmt = conn.prepare(
                "SELECT code, name FROM characters"
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut vec = Vec::new();
            for row in rows {
                vec.push(row?);
            }
            Ok::<_, rusqlite::Error>(vec)
        })();
        
        if let Ok(chars) = result {
            for (code, name) in chars {
                let clean_name = strip_renpy_tags(&name);
                if !clean_name.is_empty() {
                    mapping.insert(code, clean_name);
                }
            }
        }
    }
    
    // Фоллбэк: парсим .rpy файлы (для случаев когда БД ещё не заполнена)
    if mapping.is_empty() {
        let game_dir = Path::new(&project_path).join("game");
        if game_dir.exists() {
            for entry in WalkDir::new(&game_dir).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("rpy") {
                    if let Ok(content) = fs::read_to_string(p) {
                        for line in content.lines() {
                            let trim_line = line.trim();
                            if trim_line.starts_with("define ") && trim_line.contains("Character") {
                                let parts: Vec<&str> = trim_line.splitn(2, '=').collect();
                                if parts.len() == 2 {
                                    let code = parts[0].replace("define", "").trim().to_string();
                                    let val = parts[1];
                                    if let Some(start) = val.find('"') {
                                        if let Some(end) = val[start+1..].find('"') {
                                            let name = strip_renpy_tags(&val[start+1..start+1+end]);
                                            mapping.insert(code, name);
                                        }
                                    } else if let Some(start) = val.find('\'') {
                                        if let Some(end) = val[start+1..].find('\'') {
                                            let name = strip_renpy_tags(&val[start+1..start+1+end]);
                                            mapping.insert(code, name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(mapping)
}

/// Удаляет Ren'Py теги из строки: {color=xxx}Text{/color} → Text
fn strip_renpy_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '{' {
            in_tag = true;
        } else if ch == '}' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

/// Ранний хук _()/translate_string — выполняется в `python early`, ДО init-фазы.
/// Нужен, т.к. image/layeredimage с Text(_("...")) (напр. вкладки меню Refuge of Embers)
/// вычисляют _() при сборке картинки на init и «запекают» результат — поздний хук (init)
/// их не перехватит. store._/__ держат прямые ссылки на translate_string (minstore),
/// поэтому переопределяем И store._/__, И атрибут модуля. Идёт под `python early:`.
const RENFORGE_EARLY_HOOK: &str = r#"    # --- Канал 6 (ранний): языконезависимая доставка _()/translate_string ---
    if _renforge_ui_map:
        try:
            _rf_store = None
            try:
                _rf_store = store
            except Exception:
                try:
                    _rf_store = renpy.store
                except Exception:
                    _rf_store = None
            _renforge_ts_orig = None
            if _rf_store is not None:
                _renforge_ts_orig = getattr(_rf_store, "_", None)
            if _renforge_ts_orig is None:
                _renforge_ts_orig = renpy.translation.translate_string
            def _renforge_translate_string(s, *args, **kwargs):
                try:
                    if s in _renforge_ui_map:
                        return _renforge_ui_map[s]
                except Exception:
                    pass
                _r = _renforge_ts_orig(s, *args, **kwargs)
                try:
                    if _r == s: _renforge_log_uncovered("ui", s)
                except Exception:
                    pass
                return _r
            if _rf_store is not None:
                try:
                    _rf_store._ = _renforge_translate_string
                except Exception:
                    pass
                try:
                    _rf_store.__ = _renforge_translate_string
                except Exception:
                    pass
            try:
                renpy.translation.translate_string = _renforge_translate_string
            except Exception:
                pass
        except Exception:
            pass
"#;

/// Рантайм-код перевода. Универсален для Ren'Py 6.x–8.x.
/// Канал 1 (диалоги/меню): config.say_menu_text_filter — перехват по тексту.
/// Канал 2 (UI): прямая запись в translator.strings[lang].translations,
///   что переопределяет даже встроенные переводы игры БЕЗ краша add().
/// Плейсхолдер {LANG} подставляется в Rust.
const RENFORGE_RUNTIME: &str = r#"    # --- Канал 1: диалоги и меню через say_menu_text_filter ---
    def _renforge_lookup(s):
        try:
            if s in _renforge_say_map:
                return _renforge_say_map[s]
        except Exception:
            pass
        try:
            if not isinstance(s, unicode):
                us = s.decode("utf-8")
                if us in _renforge_say_map:
                    return _renforge_say_map[us]
        except Exception:
            pass
        _renforge_log_uncovered("say", s)
        return s

    try:
        _renforge_prev_filter = config.say_menu_text_filter
    except Exception:
        _renforge_prev_filter = None

    def _renforge_filter(s):
        if _renforge_prev_filter is not None:
            try:
                s = _renforge_prev_filter(s)
            except Exception:
                pass
        return _renforge_lookup(s)

    config.say_menu_text_filter = _renforge_filter

    # --- Канал 2: UI-строки через прямую инъекцию в StringTranslator ---
    def _renforge_inject_ui():
        try:
            translator = renpy.game.script.translator
        except Exception:
            return
        try:
            stl = translator.strings["{LANG}"]
        except Exception:
            return
        for _o, _t in _renforge_ui_map.items():
            try:
                # Прямая запись минует add() и его проверку коллизий -> без краша.
                stl.translations[_o] = _t
            except Exception:
                pass

    # Регистрируем инъекцию на момент, когда транслятор уже построен.
    try:
        _renforge_inject_ui()
    except Exception:
        pass
    try:
        config.start_callbacks.append(_renforge_inject_ui)
    except Exception:
        pass

    # --- Канал 6 (хук translate_string + store._/__) вынесен в python early ---
    # (RENFORGE_EARLY_HOOK) — иначе layeredimage/image с Text(_("...")) на init-фазе
    # вычисляют _() ДО установки хука и «запекают» оригинал.

    # --- Канал 3: промпты renpy.input (имя игрока и т.п.) ---
    # Движок не переводит сырой промпт (автор обычно без _()), поэтому оборачиваем
    # renpy.input и подменяем промпт по нашим словарям ДО подстановки [var].
    def _renforge_input_lookup(p):
        try:
            if p in _renforge_ui_map:
                return _renforge_ui_map[p]
        except Exception:
            pass
        try:
            if p in _renforge_say_map:
                return _renforge_say_map[p]
        except Exception:
            pass
        _renforge_log_uncovered("input", p)
        return p

    try:
        _renforge_input_orig = renpy.exports.input
        def _renforge_input(prompt="", *args, **kwargs):
            try:
                prompt = _renforge_input_lookup(prompt)
            except Exception:
                pass
            return _renforge_input_orig(prompt, *args, **kwargs)
        renpy.exports.input = _renforge_input
        renpy.input = _renforge_input
    except Exception:
        pass

    # --- Канал 5: легаси SL1-UI через обёртку renpy.ui.text ---
    # Древние движки (6.12–6.17) НЕ имеют фреймворка переводов: модуль
    # renpy.translation отсутствует, translator.strings недоступен, поэтому
    # Канал 2 на них мёртв. Текст экранов там рисуется прямым вызовом ui.text(...),
    # а ui.textbutton/ui.label внутри зовут тот же глобальный text(). Подменяем
    # первый строковый аргумент по _renforge_ui_map — полностью языконезависимо
    # и без зависимости от системы переводов. На современных движках безвредно
    # (точное совпадение оригинала; SL2-экраны идут своим путём).
    try:
        _rf_strtypes = (str, unicode)
    except NameError:
        _rf_strtypes = (str,)

    def _renforge_ui_text_lookup(s):
        try:
            if isinstance(s, _rf_strtypes) and s in _renforge_ui_map:
                return _renforge_ui_map[s]
        except Exception:
            pass
        _renforge_log_uncovered("uitext", s)
        return s

    try:
        import renpy.ui as _rf_ui_mod
        if _renforge_ui_map:
            _renforge_ui_text_orig = _rf_ui_mod.text
            def _renforge_ui_text(s=None, *args, **kwargs):
                return _renforge_ui_text_orig(_renforge_ui_text_lookup(s), *args, **kwargs)
            _rf_ui_mod.text = _renforge_ui_text
    except Exception:
        pass
"#;

/// Диагностика покрытия (opt-in): no-op заглушка `_renforge_log_uncovered`. Шаблоны каналов
/// всегда зовут её на промахах; когда диагностика выключена — она ничего не делает.
const RENFORGE_LOG_NOOP: &str = "    def _renforge_log_uncovered(chan, s):\n        pass\n";

/// Диагностика покрытия (opt-in): реальный логгер. На промахе канала пишет строку (с дедупом
/// в рамках сессии) в `<basedir>/renforge_uncovered.log` как `chan\t<escaped>`. RenForge затем
/// сверяет лог с БД и показывает, что видно в игре, но не покрыто. Всё в try/except — не падает.
const RENFORGE_LOG_DIAG: &str = r#"    # --- Диагностика покрытия: лог непокрытого текста ---
    _renforge_uncov_seen = set()
    def _renforge_log_uncovered(chan, s):
        try:
            _st = (str, unicode)
        except NameError:
            _st = (str,)
        try:
            if not isinstance(s, _st) or not s:
                return
            _k = (chan, s)
            if _k in _renforge_uncov_seen:
                return
            _renforge_uncov_seen.add(_k)
            _e = s.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")
            import os as _os
            _p = _os.path.join(config.basedir, "renforge_uncovered.log")
            _f = open(_p, "ab")
            _f.write((chan + "\t" + _e + "\n").encode("utf-8"))
            _f.close()
        except Exception:
            pass
"#;

/// API для пользовательских хуков, доступный в фазе `python early` (словари уже определены).
const RENFORGE_HOOK_API_EARLY: &str = r#"    # --- RenForge: API для пользовательских хуков ---
    def renforge_tr(s):
        try:
            if s in _renforge_ui_map: return _renforge_ui_map[s]
            if s in _renforge_say_map: return _renforge_say_map[s]
        except Exception:
            pass
        return s
    def renforge_add(orig, tran, ui=True):
        try:
            (_renforge_ui_map if ui else _renforge_say_map)[orig] = tran
        except Exception:
            pass
"#;

/// API обёрток, доступный в фазе `init 1000` (renpy/config/_rf_strtypes уже готовы).
const RENFORGE_HOOK_API_INIT: &str = r#"    # --- RenForge: API обёрток для пользовательских хуков ---
    def _rf_resolve(dotted):
        parts = dotted.split(".")
        if parts[0] == "renpy":
            obj = renpy; chain = parts[1:]
        elif parts[0] == "store":
            obj = renpy.store; chain = parts[1:]
        else:
            obj = renpy.store; chain = parts
        parent = None; attr = None
        for p in chain:
            parent = obj; attr = p; obj = getattr(obj, p)
        return parent, attr, obj
    def renforge_wrap(dotted, arg=0):
        try:
            parent, attr, fn = _rf_resolve(dotted)
            if getattr(fn, "_renforge_wrapped", False): return True
            def _w(*a, **k):
                a = list(a)
                try:
                    if len(a) > arg and isinstance(a[arg], _rf_strtypes):
                        a[arg] = renforge_tr(a[arg])
                except Exception:
                    pass
                return fn(*a, **k)
            try: _w._renforge_wrapped = True
            except Exception: pass
            setattr(parent, attr, _w)
            return True
        except Exception:
            return False
    def renforge_wrap_ret(dotted):
        try:
            parent, attr, fn = _rf_resolve(dotted)
            def _w(*a, **k):
                r = fn(*a, **k)
                try:
                    if isinstance(r, _rf_strtypes): return renforge_tr(r)
                except Exception:
                    pass
                return r
            setattr(parent, attr, _w)
            return True
        except Exception:
            return False
    def renforge_filter(func):
        try:
            _prev = config.say_menu_text_filter
            def _chain(s):
                if _prev is not None:
                    try: s = _prev(s)
                    except Exception: pass
                try: return func(s)
                except Exception: return s
            config.say_menu_text_filter = _chain
        except Exception:
            pass
"#;

/// Реиндент пользовательского кода: префиксует каждую непустую строку нужным числом пробелов.
fn indent_block(code: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    code.replace("\r\n", "\n")
        .split('\n')
        .map(|l| if l.trim().is_empty() { String::new() } else { format!("{}{}", pad, l) })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Глобальный путь хранения хуков (общий для всех проектов): %APPDATA%/RenForge/.
fn global_hooks_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").ok().map(std::path::PathBuf::from)
        .or_else(|| std::env::var("XDG_CONFIG_HOME").ok().map(std::path::PathBuf::from))
        .or_else(|| std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("RenForge").join("delivery_hooks.json")
}

/// Путь хранения хуков конкретного проекта.
fn project_hooks_path(project_path: &str) -> std::path::PathBuf {
    std::path::Path::new(project_path).join(".renforge").join("delivery_hooks.json")
}

/// Читает хуки из файла и проставляет каждому область (scope).
fn read_hooks_file(path: &std::path::Path, scope: &str) -> Vec<crate::models::DeliveryHook> {
    match std::fs::read_to_string(path) {
        Ok(s) => {
            let mut v: Vec<crate::models::DeliveryHook> = serde_json::from_str(&s).unwrap_or_default();
            for h in v.iter_mut() { h.scope = Some(scope.to_string()); }
            v
        }
        Err(_) => Vec::new(),
    }
}

/// Загружает все хуки доставки: сначала глобальные (общие приёмы), затем проектные.
fn load_delivery_hooks(project_path: &str) -> Vec<crate::models::DeliveryHook> {
    let mut out = read_hooks_file(&global_hooks_path(), "global");
    out.extend(read_hooks_file(&project_hooks_path(project_path), "project"));
    out
}

/// Вплетает включённые хуки заданной фазы в блок (с реиндентом и try/except-песочницей).
fn weave_hooks(out: &mut String, hooks: &[crate::models::DeliveryHook], phase: &str) {
    for h in hooks.iter().filter(|h| h.enabled && h.phase == phase && !h.code.trim().is_empty()) {
        let safe_name = h.name.replace('\n', " ").replace('\r', " ");
        // Табы -> 4 пробела: иначе наш реиндент пробелами даст смешение (TabError в игре),
        // которое валидатор сырого кода не ловит.
        let code = h.code.replace('\t', "    ");
        out.push_str(&format!("    # --- RenForge user hook: {} ---\n", safe_name));
        out.push_str("    try:\n");
        out.push_str(&indent_block(&code, 8));
        out.push_str("\n    except Exception:\n        pass\n");
    }
}

/// Извлекает интерполяции вида [var] из строки (для защиты от чужих переменных).
fn extract_interps(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if c == '[' {
            start = Some(i);
        } else if c == ']' {
            if let Some(st) = start {
                out.push(s[st..=i].to_string());
                start = None;
            }
        }
    }
    out
}

/// Экранирование строки для Python-литерала внутри u"..." (словарь доставки).
/// U+2028/U+2029 (Unicode line/paragraph separators) экранируем в `\uXXXX`: сам CPython их
/// в литерале терпит, НО это разделители строк для `str.splitlines()` и лексеров, читающих
/// .rpy построчно (Ren'Py) — сырой символ мог бы «разорвать» логическую строку и сломать
/// разбор ВСЕГО файла доставки. `\u2028` даёт ТОТ ЖЕ рантайм-символ → значение перевода не
/// меняется, безопасным становится только исходное представление. (Порядок важен: замену
/// `\\` делаем первой, поэтому вводимые здесь бэкслэши повторно не экранируются.)
fn escape_py_double(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('\"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
     .replace('\u{2028}', "\\u2028")
     .replace('\u{2029}', "\\u2029")
}

/// Рекурсивно снимает атрибут «только чтение» с файлов/папок проекта.
/// Read-only ломает запись на шагах распаковки (.rpa), декомпиляции (.rpy),
/// генерации tl/ движком и патча. Вызываем перед всеми пишущими шагами.
pub fn clear_readonly_recursive(root: &Path) {
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if let Ok(metadata) = entry.metadata() {
            let mut perms = metadata.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                let _ = std::fs::set_permissions(entry.path(), perms);
            }
        }
    }
}

#[tauri::command]
fn prepare_writable(project_path: String) -> Result<(), String> {
    clear_readonly_recursive(Path::new(&project_path));
    Ok(())
}

/// Проба записи в папку проекта (game/, иначе корень). Возвращает false, если запись
/// невозможна — типичный случай: игра в Program Files/Steam под защитой UAC. Фронт
/// тогда просит скопировать игру в пользовательскую папку или запустить от админа.
#[tauri::command]
fn is_path_writable(project_path: String) -> bool {
    let game = Path::new(&project_path).join("game");
    let dir = if game.exists() { game } else { Path::new(&project_path).to_path_buf() };
    let probe = dir.join(".renforge_write_test.tmp");
    match std::fs::write(&probe, b"renforge") {
        Ok(_) => { let _ = std::fs::remove_file(&probe); true }
        Err(_) => false,
    }
}

/// Счётчики результата сборки перевода — для локализованного отчёта на фронте.
#[derive(serde::Serialize, Default)]
pub struct BuildCounts {
    /// ключей в say_map (диалоги/меню + их alt-варианты)
    pub say: usize,
    /// ключей в strings_map (UI/python + alt-варианты)
    pub ui: usize,
    /// из доставленных — со статусом 'outdated' (перенос/память): требуют проверки
    pub review: usize,
    /// пропущено из-за чужой интерполяции [var]
    pub skipped_bad: usize,
}

#[tauri::command]
fn generate_translations(project_path: String, target_lang: String, diagnostic: bool) -> Result<BuildCounts, String> {
    generate_translations_core(project_path, target_lang, diagnostic)
}

/// Экспертный просмотр: содержимое нашего рантайм-файла перевода («наша вёрстка»).
/// Если мод уже собран — отдаём реальный game/renforge_translations.rpy; иначе строим превью.
#[tauri::command]
fn preview_generated_translations(project_path: String, target_lang: String) -> Result<String, String> {
    let built = std::path::Path::new(&project_path).join("game").join("renforge_translations.rpy");
    if built.exists() {
        if let Ok(s) = std::fs::read_to_string(&built) {
            return Ok(s);
        }
    }
    let (out, _) = build_runtime_rpy(&project_path, &target_lang, false)?;
    Ok(out)
}

/// Экспертный просмотр: декомпиляция исходного .rpyc через unrpyc (read-only, в кэш
/// .renforge/decomp/, БЕЗ записи рядом с оригиналом). Если рядом лежит loose .rpy —
/// отдаём его напрямую (это и есть исходник, декомпиляция не нужна).
#[tauri::command]
async fn decompile_rpyc(app: tauri::AppHandle, project_path: String, file_path: String) -> Result<String, String> {
    let game = std::path::Path::new(&project_path).join("game");
    let rel = file_path.trim();
    let stem = rel.strip_suffix(".rpyc").or_else(|| rel.strip_suffix(".rpy")).unwrap_or(rel);
    let rpyc = game.join(format!("{}.rpyc", stem));
    let rpy_loose = game.join(format!("{}.rpy", stem));

    // Loose .rpy рядом = реальный исходник игры → отдаём как есть.
    if rpy_loose.exists() {
        return std::fs::read_to_string(&rpy_loose).map_err(|e| e.to_string());
    }
    if !rpyc.exists() {
        return Err("rpyc_missing".to_string());
    }

    let cache = std::path::Path::new(&project_path).join(".renforge").join("decomp").join(format!("{}.rpy", stem));
    if cache.exists() {
        if let Ok(s) = std::fs::read_to_string(&cache) {
            return Ok(s);
        }
    }
    if let Some(parent) = cache.parent() { let _ = std::fs::create_dir_all(parent); }

    let sidecar = app.shell().sidecar("rpyc_extractor").map_err(|e| e.to_string())?;
    let output = sidecar
        .arg("--decompile").arg(&rpyc)
        .arg("--out").arg(&cache)
        .output()
        .await
        .map_err(|e| format!("decompile_spawn_failed: {}", e))?;
    if !output.status.success() {
        return Err(format!("decompile_error:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    std::fs::read_to_string(&cache).map_err(|e| e.to_string())
}

/// Экспертный режим: чтение пользовательских хуков доставки (глобальные + проектные).
#[tauri::command]
fn get_delivery_hooks(project_path: String) -> Result<Vec<crate::models::DeliveryHook>, String> {
    Ok(load_delivery_hooks(&project_path))
}

/// Экспертный режим: сохранение хуков. Разбивает по области: global → AppData, project → .renforge.
#[tauri::command]
fn save_delivery_hooks(project_path: String, hooks: Vec<crate::models::DeliveryHook>) -> Result<(), String> {
    let mut global: Vec<&crate::models::DeliveryHook> = Vec::new();
    let mut project: Vec<&crate::models::DeliveryHook> = Vec::new();
    for h in &hooks {
        if h.scope.as_deref() == Some("global") { global.push(h); } else { project.push(h); }
    }
    // Записываем без поля scope (оно определяется файлом).
    let strip = |list: &[&crate::models::DeliveryHook]| -> String {
        let clean: Vec<serde_json::Value> = list.iter().map(|h| serde_json::json!({
            "name": h.name, "phase": h.phase, "enabled": h.enabled, "code": h.code,
        })).collect();
        serde_json::to_string_pretty(&clean).unwrap_or_else(|_| "[]".to_string())
    };

    let gpath = global_hooks_path();
    if let Some(dir) = gpath.parent() { let _ = std::fs::create_dir_all(dir); }
    std::fs::write(&gpath, strip(&global)).map_err(|e| e.to_string())?;

    let ppath = project_hooks_path(&project_path);
    if let Some(dir) = ppath.parent() { let _ = std::fs::create_dir_all(dir); }
    std::fs::write(&ppath, strip(&project)).map_err(|e| e.to_string())?;

    // Хуки вплетаются при генерации -> помечаем собранный мод как устаревший.
    if let Ok(conn) = crate::db::get_db_conn(&project_path) {
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT)", []);
        let _ = conn.execute("INSERT OR REPLACE INTO project_meta (key, value) VALUES ('built_dirty', '1')", []);
    }
    Ok(())
}

/// Экспертный режим: проверка синтаксиса кода хука через Python-сайдкар (compile()).
#[tauri::command]
async fn validate_delivery_hook(app: tauri::AppHandle, project_path: String, code: String) -> Result<(), String> {
    let dir = std::path::Path::new(&project_path).join(".renforge");
    let _ = std::fs::create_dir_all(&dir);
    let tmp = dir.join("_hook_check.py");
    std::fs::write(&tmp, code.as_bytes()).map_err(|e| e.to_string())?;
    let sidecar = app.shell().sidecar("rpyc_extractor").map_err(|e| e.to_string())?;
    let output = sidecar
        .arg("--check-syntax").arg(&tmp)
        .output().await
        .map_err(|e| format!("check_spawn_failed: {}", e))?;
    let _ = std::fs::remove_file(&tmp);
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Чистая логика генерации (без tauri::command) — для CLI/тестов.
pub fn generate_translations_core(project_path: String, target_lang: String, diagnostic: bool) -> Result<BuildCounts, String> {
    let (out, counts) = build_runtime_rpy(&project_path, &target_lang, diagnostic)?;
    let game_dir = std::path::Path::new(&project_path).join("game");

    // Чистим старые сгенерированные файлы (в т.ч. из прошлых версий RenForge).
    let tl_dir = game_dir.join("tl").join(&target_lang);
    for stale in &["renforge_dialogue.rpy", "renforge_dialogue.rpyc",
                   "renforge_strings.rpy", "renforge_strings.rpyc",
                   "_renforge_strings.rpy", "_renforge_strings.rpyc"] {
        let p = tl_dir.join(stale);
        if p.exists() { let _ = std::fs::remove_file(&p); }
    }
    for stale in &["renforge_translations.rpy", "renforge_translations.rpyc"] {
        let p = game_dir.join(stale);
        if p.exists() { let _ = std::fs::remove_file(&p); }
    }

    std::fs::write(game_dir.join("renforge_translations.rpy"), out)
        .map_err(|e| format!("Ошибка записи перевода: {}", e))?;

    Ok(counts)
}

/// Строит содержимое рантайм-файла перевода (БЕЗ записи/чистки) — общий код для
/// генерации и для превью «нашей вёрстки» в экспертном просмотре.
fn build_runtime_rpy(project_path: &str, target_lang: &str, diagnostic: bool) -> Result<(String, BuildCounts), String> {
    let conn = crate::db::get_db_conn(project_path).map_err(|e| e.to_string())?;

    // Берём переведённые И перенесённые-на-проверку (status='outdated') строки: теперь
    // доставляем и те, и другие — перенос/память видны в игре (иначе их нельзя проверить
    // по Shift+R); статус несём для отчёта «требуют проверки». block_type — канал доставки:
    //   dialogue / menu  -> say_menu_text_filter (перехват по тексту say/menu)
    //   ui / python      -> прямая запись в translator.strings[lang].translations
    // Оба канала — рантайм, без translate-блоков и без ID. Это исключает
    // коллизии со встроенными переводами игры и краши "translation already exists".
    let mut stmt = conn.prepare(
        "SELECT block_type, original, translation, channel, alt_texts, status
         FROM translations
         WHERE status IN ('translated', 'outdated') AND translation IS NOT NULL AND translation != ''"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    }).map_err(|e| e.to_string())?;

    let records: Vec<(String, String, String, Option<String>, Option<String>, String)> = rows.filter_map(Result::ok).collect();
    if records.is_empty() {
        return Err("В базе нет сохраненных переводов!".to_string());
    }

    // Разделяем по каналам, дедуплицируем по оригиналу.
    use std::collections::HashMap as StdHashMap;
    let mut say_map: StdHashMap<String, String> = StdHashMap::new();    // диалоги + меню
    let mut strings_map: StdHashMap<String, String> = StdHashMap::new(); // ui + python
    let mut review_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut skipped_bad = 0usize;

    for (block_type, original, translation, channel, alt_texts, status) in &records {
        if original.is_empty() { continue; }
        // Идентичная пара (перевод == оригиналу) — подмена текста самим собой, no-op.
        // Это и подтверждённые вручную строки («…»→«…», имена, числа): в учёте они
        // «переведены», но доставлять их незачем — пропускаем, чтобы не раздувать файл.
        if original == translation { continue; }
        // Предохранитель: пропускаем перевод с интерполяцией [var], которой нет в
        // оригинале (несуществующая переменная -> KeyError в игре, как с "[ПЕР]").
        // Редактор это подсвечивает, но импорт/ИИ могли записать напрямую.
        {
            let orig_i = extract_interps(original);
            if extract_interps(translation).into_iter().any(|t| !orig_i.contains(&t)) {
                skipped_bad += 1;
                continue;
            }
        }
        // Канал доставки: override (say|ui|both) поверх авто-выбора по block_type.
        // auto: dialogue/menu -> say_map (say_menu_text_filter), остальное -> ui_map.
        let chan = channel.as_deref().unwrap_or("").trim();
        let (to_say, to_ui) = match chan {
            "say"  => (true, false),
            "ui"   => (false, true),
            "both" => (true, true),
            _ => match block_type.as_str() {
                "dialogue" | "menu" => (true, false),
                _ => (false, true),
            },
        };
        // status='outdated' (перенос/память) теперь тоже доставляем, но помечаем на проверку.
        let is_review = status == "outdated";
        if to_say && !say_map.contains_key(original) {
            say_map.insert(original.clone(), translation.clone());
            if is_review { review_keys.insert(original.clone()); }
        }
        if to_ui && !strings_map.contains_key(original) {
            strings_map.insert(original.clone(), translation.clone());
            if is_review { review_keys.insert(original.clone()); }
        }

        // Multi-key: тот же перевод — под альтернативными текстами строки (варианты в
        // языке-источнике, напр. base + tl/english). Рантайм показывает один из них,
        // мы ключуем все → строка матчится в любом случае. alt_texts — JSON-массив.
        if let Some(alt_json) = alt_texts {
            if let Ok(alts) = serde_json::from_str::<Vec<String>>(alt_json) {
                let tr_interps = extract_interps(translation);
                for alt in &alts {
                    if alt.is_empty() || alt == original || alt == translation { continue; }
                    // Предохранитель KeyError: перевод не должен требовать [var], которых
                    // нет в alt-ключе (реворд мог отбросить переменную).
                    let alt_i = extract_interps(alt);
                    if tr_interps.iter().any(|t| !alt_i.contains(t)) { continue; }
                    if to_say {
                        say_map.entry(alt.clone()).or_insert_with(|| translation.clone());
                    }
                    if to_ui {
                        strings_map.entry(alt.clone()).or_insert_with(|| translation.clone());
                    }
                }
            }
        }
    }

    // === Единый рантайм-файл перевода ===
    let mut out = String::from("# RenForge generated translations (runtime, crash-safe)\n");

    // python early: словари + РАННИЙ хук _()/translate_string. Ставим до init-фазы,
    // потому что image/layeredimage с Text(_("...")) (напр. вкладки меню Refuge of Embers:
    // `Text(_("START"), style="tabs")`) вычисляют _() при сборке картинки на init и
    // «запекают» результат. Поздний хук (init) UI-текст в картинках уже не перехватит.
    out.push_str("python early:\n");

    // 1) Словарь диалогов/меню
    out.push_str("    _renforge_say_map = {\n");
    for (orig, trans) in &say_map {
        out.push_str(&format!("        u\"{}\": u\"{}\",\n",
            escape_py_double(orig), escape_py_double(trans)));
    }
    out.push_str("    }\n");

    // 2) Словарь UI-строк
    out.push_str("    _renforge_ui_map = {\n");
    for (orig, trans) in &strings_map {
        out.push_str(&format!("        u\"{}\": u\"{}\",\n",
            escape_py_double(orig), escape_py_double(trans)));
    }
    out.push_str("    }\n");

    // 2.5) Диагностика покрытия (opt-in): всегда определяем `_renforge_log_uncovered` — реальный
    // логгер при диагностической сборке, иначе no-op. Шаблоны каналов зовут его на промахах.
    out.push_str(if diagnostic { RENFORGE_LOG_DIAG } else { RENFORGE_LOG_NOOP });

    // 3) Ранний языконезависимый хук _()/translate_string + store._/__
    out.push_str(RENFORGE_EARLY_HOOK);
    out.push_str("\n");

    // 3.5) Пользовательские хуки доставки (экспертный режим), фаза early.
    let hooks = load_delivery_hooks(project_path);
    let has_hooks = hooks.iter().any(|h| h.enabled && !h.code.trim().is_empty());
    if has_hooks {
        out.push_str(RENFORGE_HOOK_API_EARLY);
        weave_hooks(&mut out, &hooks, "early");
        out.push_str("\n");
    }

    // init 1000: каналы, которые должны победить init игры (say_menu_text_filter)
    // или которым нужен построенный транслятор (инъекция в translator.strings) + input-хук.
    out.push_str("init 1000 python:\n");
    out.push_str(&RENFORGE_RUNTIME.replace("{LANG}", target_lang));

    // Пользовательские хуки доставки, фаза init (после наших каналов — API/словари готовы).
    if has_hooks {
        out.push_str("\n");
        out.push_str(RENFORGE_HOOK_API_INIT);
        weave_hooks(&mut out, &hooks, "init");
    }

    Ok((out, BuildCounts { say: say_map.len(), ui: strings_map.len(), review: review_keys.len(), skipped_bad }))
}


/// Одна строка из лога непокрытого (диагностика покрытия). `in_db` — есть ли оригинал в БД
/// активной пары; `translated` — переведён ли он. Ценные — те, что НЕ в БД: их извлечение/
/// доставка не покрыла, кандидаты в ручные строки.
#[derive(serde::Serialize)]
pub struct UncoveredEntry {
    pub chan: String,       // say | ui | input | uitext
    pub text: String,
    pub in_db: bool,
    pub translated: bool,
}

/// Разэкранирование строки из лога (`\\`, `\n`, `\r`, `\t`).
fn unescape_log(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(o) => { out.push('\\'); out.push(o); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Диагностика покрытия: читает `<project>/renforge_uncovered.log` (пишется рантайм-логгером
/// при диагностической сборке), дедуплицирует и сверяет с БД активной пары. Возвращает строки,
/// ВИДИМЫЕ в игре; те, что НЕ в БД (`in_db=false`) — кандидаты (извлечение их не поймало).
#[tauri::command]
fn read_uncovered(project_path: String) -> Result<Vec<UncoveredEntry>, String> {
    let log_path = std::path::Path::new(&project_path).join("renforge_uncovered.log");
    let content = match std::fs::read_to_string(&log_path) {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    // Оригиналы из БД: original -> есть ли непустой перевод.
    let mut db: HashMap<String, bool> = HashMap::new();
    if let Ok(conn) = crate::db::get_db_conn(&project_path) {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT original, MAX(CASE WHEN status='translated' AND translation IS NOT NULL AND translation != '' THEN 1 ELSE 0 END) \
             FROM translations GROUP BY original"
        ) {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1).unwrap_or(0) == 1))) {
                for row in rows.flatten() { db.insert(row.0, row.1); }
            }
        }
    }
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in content.lines() {
        if line.is_empty() { continue; }
        let mut parts = line.splitn(2, '\t');
        let chan = parts.next().unwrap_or("").to_string();
        let text = match parts.next() { Some(e) => unescape_log(e), None => continue };
        if !seen.insert((chan.clone(), text.clone())) { continue; }
        let (in_db, translated) = match db.get(&text) {
            Some(&t) => (true, t),
            None => (false, false),
        };
        out.push(UncoveredEntry { chan, text, in_db, translated });
    }
    Ok(out)
}

/// Диагностика покрытия: удалить лог непокрытого (сброс перед новым прогоном).
#[tauri::command]
fn clear_uncovered(project_path: String) -> Result<(), String> {
    let log_path = std::path::Path::new(&project_path).join("renforge_uncovered.log");
    if log_path.exists() {
        std::fs::remove_file(&log_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn apply_renforge_patch(
    project_path: String, 
    target_lang: String, 
    font_remaps: Vec<crate::models::FontRemap>
) -> Result<(), String> {
    apply_renforge_patch_core(project_path, target_lang, font_remaps)
}

/// Список шрифтов игры с пометкой наличия кириллицы (для UI поштучной подмены).
#[tauri::command]
fn get_project_fonts(project_path: String) -> Result<Vec<crate::models::FontInfo>, String> {
    let game_dir = Path::new(&project_path).join("game");
    Ok(list_game_fonts(&game_dir))
}

/// Сканирует game/ на .ttf/.otf и проверяет наличие кириллических глифов через ttf-parser.
pub fn list_game_fonts(game_dir: &Path) -> Vec<crate::models::FontInfo> {
    let mut fonts = Vec::new();
    for entry in walkdir::WalkDir::new(game_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() { continue; }
        let ext = match p.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
            Some(e) => e,
            None => continue,
        };
        if ext != "ttf" && ext != "otf" { continue; }
        let rel = match p.strip_prefix(game_dir) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        // Пропускаем шрифты, которые мы сами скопировали при сборке мода
        if rel == "renforge_font.ttf" || rel.starts_with("renforge_font_") { continue; }
        let scripts = font_scripts(p);
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
        fonts.push(crate::models::FontInfo { rel_path: rel, name, scripts });
    }
    fonts.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    fonts
}

/// Определяет, какие письменности покрывает шрифт, по нескольким типовым глифам
/// каждого скрипта. Возвращает список кодов покрытых скриптов.
fn font_scripts(path: &Path) -> Vec<String> {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let face = match ttf_parser::Face::parse(&data, 0) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    // Для каждого скрипта — набор проб; считаем покрытым, если есть ВСЕ пробы.
    let probes: &[(&str, &[char])] = &[
        ("latin", &['A', 'z']),
        ("vietnamese", &['\u{01A1}', '\u{1EA7}']), // ơ, ầ
        ("cyrillic", &['\u{0410}', '\u{044F}']),   // А, я
        ("greek", &['\u{0391}', '\u{03C9}']),      // Α, ω
        ("armenian", &['\u{0531}', '\u{0561}']),   // Ա, ա
        ("georgian", &['\u{10D0}', '\u{10D1}']),   // ა, ბ
        ("hebrew", &['\u{05D0}', '\u{05EA}']),     // א, ת
        ("arabic", &['\u{0627}', '\u{0628}']),     // ا, ب
        ("devanagari", &['\u{0905}', '\u{0915}']), // अ, क
        ("bengali", &['\u{0985}', '\u{0995}']),    // অ, ক
        ("gurmukhi", &['\u{0A05}', '\u{0A15}']),   // ਅ, ਕ
        ("gujarati", &['\u{0A85}', '\u{0A95}']),   // અ, ક
        ("tamil", &['\u{0B85}', '\u{0B95}']),      // அ, க
        ("telugu", &['\u{0C05}', '\u{0C15}']),     // అ, క
        ("kannada", &['\u{0C85}', '\u{0C95}']),    // ಅ, ಕ
        ("malayalam", &['\u{0D05}', '\u{0D15}']),  // അ, ക
        ("sinhala", &['\u{0D85}', '\u{0D9A}']),    // අ, ක
        ("thai", &['\u{0E01}', '\u{0E17}']),       // ก, ท
        ("lao", &['\u{0E81}', '\u{0E82}']),        // ກ, ຂ
        ("tibetan", &['\u{0F40}', '\u{0F41}']),    // ཀ, ཁ
        ("myanmar", &['\u{1000}', '\u{1001}']),    // က, ခ
        ("khmer", &['\u{1780}', '\u{1781}']),      // ក, ខ
        ("ethiopic", &['\u{1200}', '\u{1208}']),   // ሀ, ለ
        ("japanese", &['\u{3042}', '\u{30AB}']),   // あ, カ (хирагана + катакана)
        ("chinese", &['\u{4E2D}', '\u{6587}']),    // 中, 文 (ханьцзы)
        ("korean", &['\u{AC00}', '\u{D55C}']),     // 가, 한 (хангыль)
    ];
    let mut out = Vec::new();
    for (code, chars) in probes {
        if chars.iter().all(|c| face.glyph_index(*c).is_some()) {
            out.push(code.to_string());
        }
    }
    out
}

/// Чистая логика патчинга (без tauri::command) — для CLI/тестов.
pub fn apply_renforge_patch_core(
    project_path: String, 
    target_lang: String, 
    font_remaps: Vec<crate::models::FontRemap>
) -> Result<(), String> {
    let root_dir = Path::new(&project_path);
    let game_dir = root_dir.join("game");
    
    // Снимаем защиту от записи (рекурсивно по проекту)
    clear_readonly_recursive(&root_dir);

    // Универсальный патч на чистом Python.
    // init -999: санация ядовитого _preferences.language.
    // ВНИМАНИЕ: язык игры НЕ переключаем намеренно. renpy.change_language(target)
    // отравляет persistent _preferences.language и ломает игры, индексирующие свои
    // словари по языку (ES: translation[..][_preferences.language] -> KeyError).
    // Доставка перевода полностью языконезависима: К1 say_menu_text_filter,
    // К5 renpy.ui.text, К6 хук translate_string — переключение языка не требуется.
    //
    // dev/console НЕ включаем: игроку это не нужно, а config.developer=True меняет пути
    // кода достижений (renpy/common/00achievement.rpy: progress() без stat_max в dev-режиме
    // бросает исключение вместо тихого return; плюс многие игры вешают на выдачу ачивок
    // `if not config.developer`). Форс dev-режима в доставленном моде ломал Steam-достижения
    // (фидбэк тестера, 1.2). Для доставки перевода dev/console не требуется.
    let mut patch_content = format!(r#"
init -999 python:
    # Санация языка ДО init-кода игры (приоритет 0). Прошлые версии патча могли
    # записать в persistent _preferences.language язык, которого игра не знает
    # (напр. "russian" у ES) — её init-код индексирует словари по языку и падает
    # KeyError ещё до нашего init 999. Сбрасываем неизвестный язык на дефолт (None);
    # доставка языконезависима (say-filter / ui.text / translate_string), так что
    # дефолтный язык ничего не ломает. Самовосстанавливает уже отравленные сохранения.
    try:
        _rf_cur = getattr(store._preferences, "language", None)
        if _rf_cur is not None:
            try:
                _rf_known = renpy.known_languages()
            except:
                _rf_known = None
            if _rf_known is not None:
                _rf_bad = _rf_cur not in _rf_known
            else:
                _rf_bad = (_rf_cur == "{target_lang}")
            if _rf_bad:
                store._preferences.language = None
    except:
        pass
"#);

    // === Шрифты: поштучный render-ремап (каждый шрифт → свой целевой) ===
    if !font_remaps.is_empty() {
        use std::collections::HashMap;
        // Встроенный кириллический шрифт движка — дефолтный target (target = None)
        let dejavu: Option<String> = {
            let mut found = None;
            for c in [
                root_dir.join("renpy").join("common").join("DejaVuSans.ttf"),
                root_dir.join("common").join("DejaVuSans.ttf"),
            ] {
                if c.exists() { found = Some(c.to_string_lossy().to_string()); break; }
            }
            found
        };
        // дедуп копий: путь-источник -> имя файла в game/
        let mut copied: HashMap<String, String> = HashMap::new();
        let mut idx = 0usize;
        let mut entries: Vec<(String, String)> = Vec::new(); // (game_font_rel, dest_name)
        for rm in &font_remaps {
            let src = match &rm.target {
                Some(p) if Path::new(p).exists() => Some(p.clone()),
                _ => dejavu.clone(),
            };
            let src = match src { Some(s) => s, None => continue };
            let dest_name = if let Some(n) = copied.get(&src) {
                n.clone()
            } else {
                let n = format!("renforge_font_{}.ttf", idx);
                idx += 1;
                let _ = fs::copy(&src, game_dir.join(&n));
                copied.insert(src.clone(), n.clone());
                n
            };
            if game_dir.join(&dest_name).exists() {
                entries.push((rm.source.clone(), dest_name));
            }
        }
        if !entries.is_empty() {
            let mut map_lines = String::new();
            for (gf, dn) in &entries {
                let gfe = gf.replace('\\', "\\\\").replace('"', "\\\"");
                let dne = dn.replace('\\', "\\\\").replace('"', "\\\"");
                map_lines.push_str(&format!("        u\"{}\": u\"{}\",\n", gfe, dne));
            }
            patch_content.push_str(&format!(r#"
    # --- Поштучный ремап шрифтов на уровне рендера (каждый -> свой) ---
    _renforge_font_map = {{
{map_lines}    }}
    try:
        for _src, _dst in _renforge_font_map.items():
            for _b in (False, True):
                for _i in (False, True):
                    config.font_replacement_map[(_src, _b, _i)] = (_dst, _b, _i)
    except: pass
"#, map_lines=map_lines));
        }
    }

    let patch_path = game_dir.join("00_renforge_patch.rpy");
    fs::write(&patch_path, patch_content).map_err(|e| format!("Ошибка создания патча: {}", e))?;

    let cache_dir = game_dir.join("cache");
    if cache_dir.exists() { let _ = fs::remove_dir_all(cache_dir); }

    // Удаляем старые скомпилированные файлы локализации, чтобы они не мешали
    let tl_dir = game_dir.join("tl").join(&target_lang);
    if tl_dir.exists() {
        for entry in walkdir::WalkDir::new(&tl_dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("rpyc") {
                let _ = fs::remove_file(p);
            }
        }
    }

    // Помечаем эту пару как собранную: её мод теперь материализован в game/.
    // Этим разблокируется экспорт перевода именно для этой пары (см. db::mark_pair_built).
    crate::db::mark_pair_built(&project_path);

    Ok(())
}

/// Рекурсивная копия файла с созданием родительских папок.
fn copy_file_mkdir(src: &Path, dest: &Path) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(src, dest).map_err(|e| format!("копия {}: {}", src.display(), e))
}

/// Имена рабочих артефактов RenForge, которые НЕ попадают в дистрибутив (служебные).
fn is_renforge_workfile(rel: &str) -> bool {
    let r = rel.replace('\\', "/");
    r == ".renforge" || r.starts_with(".renforge/") || r.contains("/.renforge/")
        || r == "renforge.db" || r.starts_with("renforge.db")
        || r == "renforge_ast.json" || r == "renforge_native.json"
        // Отключённый оригинальный архив (наш бэкап после распаковки .rpa) — в полной игре
        // он мёртвый груз: контент уже лежит распакованным. Игре он не нужен.
        || r.ends_with(".renforge-disabled")
        // Проба записи (обычно удаляется сразу, но могла остаться после краша).
        || r.ends_with(".renforge_write_test.tmp")
}

/// Экспорт «Простой путь»: копия всей игры с уже впечённым модом (минус служебные
/// файлы RenForge). Пользователю не нужно думать о версии — он получает готовую игру.
/// Глобальный флаг отмены экспорта (ставится командой cancel_export, проверяется в цикле копии).
static EXPORT_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Запросить отмену текущего экспорта. Фактическая остановка — в ближайшей итерации копии.
#[tauri::command]
fn cancel_export() {
    EXPORT_CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Результат экспорта для локализации на фронте. code: "done" | "exists" | "nospace" | "cancelled".
#[derive(serde::Serialize)]
pub struct ExportResult {
    pub code: String,
    pub files: usize,
    pub mb: f64,
    pub skipped: usize,
    pub need_gb: f64,
    pub avail_gb: f64,
}

fn export_full(app: &tauri::AppHandle, root: &Path, out_root: &Path, _target_lang: &str) -> Result<ExportResult, String> {
    use tauri::Emitter;
    std::fs::create_dir_all(out_root).map_err(|e| e.to_string())?;
    // Сначала дёшево считаем число файлов и суммарный размер (для прогресса и проверки места).
    let mut total = 0usize;
    let mut need_bytes = 0u64;
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        let rel = match p.strip_prefix(root) { Ok(r) => r, Err(_) => continue };
        let rel_str = rel.to_string_lossy().to_string();
        if rel_str.is_empty() || is_renforge_workfile(&rel_str) { continue; }
        if p.starts_with(out_root) { continue; }
        if p.is_file() {
            total += 1;
            if let Ok(m) = entry.metadata() { need_bytes += m.len(); }
        }
    }

    // Проверка свободного места на целевом диске (+5% запас). Иначе — частичная копия.
    if let Ok(avail) = fs2::available_space(out_root) {
        let need_with_margin = need_bytes.saturating_add(need_bytes / 20);
        if avail < need_with_margin {
            return Ok(ExportResult {
                code: "nospace".into(), files: 0, mb: 0.0, skipped: 0,
                need_gb: need_bytes as f64 / 1_073_741_824.0,
                avail_gb: avail as f64 / 1_073_741_824.0,
            });
        }
    }

    let _ = app.emit("export_progress", serde_json::json!({"done": 0usize, "total": total}));

    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut skipped = 0usize;
    let mut last_emit = std::time::Instant::now();
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        // Отмена: чистим недокопированную папку (частичная копия бесполезна) и выходим.
        if EXPORT_CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = std::fs::remove_dir_all(out_root);
            return Ok(ExportResult { code: "cancelled".into(), files, mb: 0.0, skipped, need_gb: 0.0, avail_gb: 0.0 });
        }
        let p = entry.path();
        let rel = match p.strip_prefix(root) { Ok(r) => r, Err(_) => continue };
        let rel_str = rel.to_string_lossy().to_string();
        if rel_str.is_empty() { continue; }
        // не копируем служебные файлы и не зацикливаемся на каталоге назначения
        if is_renforge_workfile(&rel_str) { continue; }
        if p.starts_with(out_root) { continue; }
        if p.is_dir() {
            let _ = std::fs::create_dir_all(out_root.join(rel));
            continue;
        }
        if p.is_file() {
            // Устойчивость к запущенной игре: залоченный/недоступный файл (sharing violation,
            // напр. открытый .exe) НЕ срывает весь экспорт — пропускаем и считаем.
            match copy_file_mkdir(p, &out_root.join(rel)) {
                Ok(n) => { bytes += n; files += 1; }
                Err(_) => { skipped += 1; }
            }
            if (files + skipped) % 64 == 0 || last_emit.elapsed().as_millis() > 200 {
                let _ = app.emit("export_progress", serde_json::json!({"done": files + skipped, "total": total}));
                last_emit = std::time::Instant::now();
            }
        }
    }
    let _ = app.emit("export_progress", serde_json::json!({"done": total, "total": total}));
    Ok(ExportResult {
        code: "done".into(), files, mb: bytes as f64 / 1_048_576.0, skipped,
        need_gb: 0.0, avail_gb: 0.0,
    })
}

/// Экспорт «Только мод»: оверлей-файлы RenForge с сохранением структуры (для тех, кто
/// положит их в game/ совместимой версии игры). Кладёт README с предупреждением о версии.
fn export_mod(app: &tauri::AppHandle, root: &Path, out_root: &Path, target_lang: &str) -> Result<ExportResult, String> {
    use tauri::Emitter;
    let game = root.join("game");
    if !game.join("00_renforge_patch.rpy").exists() {
        return Err("Мод не собран: нет game/00_renforge_patch.rpy. Сначала соберите мод.".to_string());
    }
    std::fs::create_dir_all(out_root).map_err(|e| e.to_string())?;
    let mut files = 0usize;

    // 1) Рантайм-файлы и патч (+ скомпилированные .rpyc, если уже есть)
    for name in ["00_renforge_patch.rpy", "00_renforge_patch.rpyc",
                 "renforge_translations.rpy", "renforge_translations.rpyc"] {
        let src = game.join(name);
        if src.exists() {
            copy_file_mkdir(&src, &out_root.join("game").join(name))?;
            files += 1;
        }
    }

    // 2) Встроенные шрифты RenForge (renforge_font_*.ttf)
    if let Ok(rd) = std::fs::read_dir(&game) {
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if fname.starts_with("renforge_font_") && fname.ends_with(".ttf") {
                copy_file_mkdir(&e.path(), &out_root.join("game").join(&fname))?;
                files += 1;
            }
        }
    }

    // 3) Локализованные ассеты (картинки/аудио) из game/tl/<target>/
    let tl = game.join("tl").join(target_lang);
    if tl.exists() {
        for entry in walkdir::WalkDir::new(&tl).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() {
                if let Ok(rel) = p.strip_prefix(root) {
                    copy_file_mkdir(p, &out_root.join(rel))?;
                    files += 1;
                }
            }
        }
    }

    // 4) README (на английском; target подставляется только в заголовок)
    let readme = format!(
        "RenForge translation ({target})\r\n\r\n\
         Contents: translation files (overlay), not the full game.\r\n\r\n\
         Installation:\r\n\
         1. Copy the game/ folder from this archive into the game directory, replacing/merging.\r\n\
         2. Launch the game - the translation is applied automatically.\r\n\r\n\
         Compatibility: the translation is built for a specific game version. Other versions\r\n\
         may show mismatches. If the version does not match, use the \"Full game\" export.\r\n\r\n\
         RenForge\r\n",
        target = target_lang
    );
    std::fs::write(out_root.join("README.txt"), readme).map_err(|e| e.to_string())?;

    let _ = app.emit("export_progress", serde_json::json!({"done": files, "total": files}));
    Ok(ExportResult { code: "done".into(), files, mb: 0.0, skipped: 0, need_gb: 0.0, avail_gb: 0.0 })
}

fn export_translation_core(app: &tauri::AppHandle, project_path: &str, target_lang: &str, mode: &str, out_root: &str, overwrite: bool) -> Result<ExportResult, String> {
    let root = Path::new(project_path);
    let out = Path::new(out_root);
    // Повторный экспорт в ту же папку: без overwrite сообщаем code "exists" (фронт спросит),
    // с overwrite — полностью очищаем целевую папку, чтобы не оставить устаревшие файлы.
    let non_empty = out.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false);
    if non_empty {
        if overwrite {
            std::fs::remove_dir_all(out).map_err(|e| format!("Не удалось очистить папку экспорта: {}", e))?;
        } else {
            return Ok(ExportResult { code: "exists".into(), files: 0, mb: 0.0, skipped: 0, need_gb: 0.0, avail_gb: 0.0 });
        }
    }
    match mode {
        "full" => export_full(app, root, out, target_lang),
        "mod" => export_mod(app, root, out, target_lang),
        _ => Err(format!("неизвестный режим экспорта: {}", mode)),
    }
}

/// Экспорт перевода для распространения. mode: "full" (вся игра) | "mod" (оверлей).
/// out_root — конечная папка (создаётся фронтом как dest/<имя>), куда складываем результат.
/// overwrite=false и непустая папка -> вернётся code "exists"; фронт спросит и повторит с true.
/// По ходу копирования шлёт события "export_progress" {done, total} для прогресс-бара.
#[tauri::command]
async fn export_translation(app: tauri::AppHandle, project_path: String, target_lang: String, mode: String, out_root: String, overwrite: bool) -> Result<ExportResult, String> {
    EXPORT_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);
    tokio::task::spawn_blocking(move || {
        export_translation_core(&app, &project_path, &target_lang, &mode, &out_root, overwrite)
    }).await.map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct StringsExportResult {
    code: String,
    files: usize,
    strings: usize,
}

struct ExportRow {
    id: String,
    file: String,
    line: i64,
    who: Option<String>,
    original: String,
    translation: String,
    status: String,
}

/// Экранирование для PO (gettext): \ " \n \t \r.
fn po_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

fn build_po(entries: &[ExportRow], target_lang: &str) -> String {
    let mut s = String::new();
    s.push_str("msgid \"\"\nmsgstr \"\"\n");
    s.push_str("\"Project-Id-Version: RenForge\\n\"\n");
    s.push_str(&format!("\"Language: {}\\n\"\n", target_lang));
    s.push_str("\"Content-Type: text/plain; charset=UTF-8\\n\"\n\n");
    for e in entries {
        let reference = e.file.replace(['\n', '\r'], " ");
        s.push_str(&format!("#: {}:{}\n", reference, e.line));
        if let Some(w) = &e.who {
            if !w.is_empty() {
                s.push_str(&format!("#. who: {}\n", w.replace(['\n', '\r'], " ")));
            }
        }
        if e.status == "outdated" {
            s.push_str("#, fuzzy\n");
        }
        s.push_str(&format!("msgctxt \"{}\"\n", po_escape(&e.id)));
        s.push_str(&format!("msgid \"{}\"\n", po_escape(&e.original)));
        s.push_str(&format!("msgstr \"{}\"\n\n", po_escape(&e.translation)));
    }
    s
}

fn build_csv(entries: &[ExportRow]) -> String {
    let mut s = String::from("ID;Original;Translation\n");
    for e in entries {
        let orig = e.original.replace('"', "\"\"").replace('\n', "[BR]");
        let mut tran = e.translation.replace('"', "\"\"").replace('\n', "[BR]");
        // Защита от CSV-инъекции формул (как в одно-файловом экспорте на фронте).
        if tran.starts_with(['=', '+', '-', '@']) {
            tran = format!("'{}", tran);
        }
        s.push_str(&format!("\"{}\";\"{}\";\"{}\"\n", e.id, orig, tran));
    }
    s
}

fn build_json(entries: &[ExportRow]) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct JsonEntry<'a> {
        id: &'a str,
        original: &'a str,
        translation: &'a str,
    }
    let v: Vec<JsonEntry> = entries
        .iter()
        .map(|e| JsonEntry { id: &e.id, original: &e.original, translation: &e.translation })
        .collect();
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

/// Безопасно склеивает out_root/<subdir>/<rel_file>.<ext>, отбрасывая `.`/`..`/абсолютные
/// компоненты (rel_file приходит из БД как путь относительно game/).
fn write_export_file(out_root: &Path, subdir: &str, rel_file: &str, ext: &str, content: &str) -> Result<(), String> {
    let mut p = out_root.join(subdir);
    for comp in rel_file.split(['/', '\\']) {
        if comp.is_empty() || comp == "." || comp == ".." {
            continue;
        }
        p.push(comp);
    }
    let leaf = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    p.set_file_name(format!("{}.{}", leaf, ext));
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, content).map_err(|e| e.to_string())
}

/// Пакетный экспорт строк активной пары во все форматы (CSV/JSON/PO) — по файлу на каждый
/// исходник, имена совпадают с исходными (script.rpy -> script.rpy.po). Несёт id строки как
/// ключ (msgctxt в PO, колонка в CSV/JSON) для будущего пакетного импорта по совпадению имён.
fn export_strings_core(project_path: &str, target_lang: &str, out_root: &str) -> Result<StringsExportResult, String> {
    let conn = crate::db::get_db_conn(project_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, line_number, who, original, translation, status
             FROM translations ORDER BY file_path, line_number",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ExportRow {
                id: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                file: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                line: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                who: r.get::<_, Option<String>>(3)?,
                original: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                translation: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                status: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;

    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<ExportRow>> = BTreeMap::new();
    let mut total = 0usize;
    for r in rows.filter_map(Result::ok) {
        total += 1;
        let key = if r.file.trim().is_empty() { "_unknown".to_string() } else { r.file.clone() };
        groups.entry(key).or_default().push(r);
    }
    if total == 0 {
        return Err("В базе нет строк для экспорта.".to_string());
    }

    let out = Path::new(out_root);
    let mut files = 0usize;
    for (file, mut entries) in groups {
        entries.sort_by_key(|e| e.line);
        write_export_file(out, "po", &file, "po", &build_po(&entries, target_lang))?;
        write_export_file(out, "csv", &file, "csv", &build_csv(&entries))?;
        write_export_file(out, "json", &file, "json", &build_json(&entries)?)?;
        files += 1;
    }
    Ok(StringsExportResult { code: "done".into(), files, strings: total })
}

#[tauri::command]
async fn export_strings(project_path: String, target_lang: String, out_root: String) -> Result<StringsExportResult, String> {
    tokio::task::spawn_blocking(move || export_strings_core(&project_path, &target_lang, &out_root))
        .await
        .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct GameFileInfo {
    rel_path: String,        // путь относительно game/, нормализован к .rpy (ключ как в БД)
    status: String,          // "extracted" | "empty" | "lang"
    total: i32,
    translated: i32,
    lang: Option<String>,    // обнаруженный языковой суффикс (если файл — чужая локализация)
}

/// Языковой суффикс файла (script_ru.rpyc -> RU). Список синхронизирован с экстрактором.
fn detect_lang_suffix(stem: &str) -> Option<String> {
    let base = stem.rsplit('/').next().unwrap_or(stem).to_lowercase();
    // Длинные суффиксы первыми (pt-br перед ...), чтобы не сматчить короткий по ошибке.
    const SUFFIXES: &[&str] = &[
        "pt-br", "zh-hant", "de", "es", "fr", "jp", "kr", "pl", "ru", "zh", "it", "nl", "sv",
        "cs", "hu", "tr", "ar", "th", "vi", "id", "uk", "ro", "bg", "hr", "en",
    ];
    for s in SUFFIXES {
        if base.ends_with(&format!("_{}", s)) {
            return Some(s.to_uppercase());
        }
    }
    None
}

/// Все скриптовые файлы игры (.rpyc/.rpy в game/) со статусом: извлечён (есть строки в БД),
/// чужой язык (по суффиксу), либо «не извлечён». Для модалки выбора пропущенного файла.
#[tauri::command]
fn list_game_files(project_path: String) -> Result<Vec<GameFileInfo>, String> {
    let game = Path::new(&project_path).join("game");
    if !game.exists() {
        return Ok(Vec::new());
    }

    // Счётчики по файлам из БД активной пары.
    let mut db_stats: HashMap<String, (i32, i32)> = HashMap::new();
    if let Ok(conn) = crate::db::get_db_conn(&project_path) {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT file_path, COUNT(id), SUM(CASE WHEN status='translated' THEN 1 ELSE 0 END)
             FROM translations GROUP BY file_path",
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, r.get::<_, Option<i32>>(2)?.unwrap_or(0)))
            }) {
                for row in rows.flatten() {
                    db_stats.insert(row.0, (row.1, row.2));
                }
            }
        }
    }

    use std::collections::BTreeMap;
    let mut seen: BTreeMap<String, GameFileInfo> = BTreeMap::new();
    for entry in walkdir::WalkDir::new(&game).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() { continue; }
        let ext = match p.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
            Some(e) => e,
            None => continue,
        };
        if ext != "rpyc" && ext != "rpy" { continue; }
        let rel = match p.strip_prefix(&game) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        // Пропускаем оверлей-переводы tl/ и наши сгенерированные файлы доставки
        // (патч 00_renforge_patch.* и renforge_*.* — это вывод RenForge, не исходники игры).
        let base = rel.rsplit('/').next().unwrap_or(&rel);
        if rel.starts_with("tl/")
            || base.starts_with("renforge_")
            || base == "00_renforge_patch.rpy" || base == "00_renforge_patch.rpyc" {
            continue;
        }
        let stem = rel.strip_suffix(".rpyc").or_else(|| rel.strip_suffix(".rpy")).unwrap_or(&rel).to_string();
        let key = format!("{}.rpy", stem);
        if seen.contains_key(&key) { continue; } // дедуп .rpyc + .rpy одного файла

        let (total, translated) = db_stats.get(&key).copied().unwrap_or((0, 0));
        let lang = detect_lang_suffix(&stem);
        let status = if total > 0 { "extracted" } else if lang.is_some() { "lang" } else { "empty" };
        seen.insert(key.clone(), GameFileInfo { rel_path: key, status: status.into(), total, translated, lang });
    }
    Ok(seen.into_values().collect())
}

/// Запрос к OpenAI-совместимому LLM API (chat/completions). Идёт через Rust (reqwest),
/// чтобы обойти CORS вебвью и не светить ключ в браузерном контексте. base_url — корень
/// API (напр. https://api.openai.com/v1 или https://openrouter.ai/api/v1); /chat/completions
/// дописывается автоматически. api_key пустой — для локальных эндпоинтов без авторизации.
#[tauri::command]
async fn llm_chat_request(
    base_url: String,
    api_key: String,
    model: String,
    system: String,
    user: String,
    temperature: f32,
) -> Result<String, String> {
    let mut url = base_url.trim().trim_end_matches('/').to_string();
    if url.is_empty() { return Err("Не указан URL API.".to_string()); }
    if !url.ends_with("/chat/completions") {
        url.push_str("/chat/completions");
    }
    let body = serde_json::json!({
        "model": model,
        "temperature": temperature,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.post(&url).json(&body);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key.trim());
    }
    let resp = req.send().await.map_err(|e| format!("Сетевая ошибка: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        let preview: String = text.chars().take(300).collect();
        format!("Некорректный ответ API (HTTP {}): {}", status.as_u16(), preview)
    })?;
    if let Some(err) = json.get("error") {
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("неизвестная ошибка API");
        return Err(format!("API: {}", msg));
    }
    let content = json.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str());
    match content {
        Some(s) => Ok(s.to_string()),
        None => Err(format!("Ответ без choices[0].message.content (HTTP {})", status.as_u16())),
    }
}

/// Удаляет внедрённый в игру мод RenForge (текст-доставку), возвращая игру к оригиналу.
/// Удаляет: патч, рантайм-перевод, встроенные шрифты RenForge, кэш; сбрасывает указатель
/// built. НЕ трогает: БД переводов (.renforge — работа сохраняется) и локализованные
/// картинки/аудио в tl/<target> (ими управляет галерея отдельными кнопками).
#[tauri::command]
fn remove_renforge_mod(project_path: String, _target_lang: String) -> Result<String, String> {
    let game = Path::new(&project_path).join("game");
    if !game.exists() { return Err("Папка game не найдена.".to_string()); }
    let mut removed = 0usize;

    clear_readonly_recursive(Path::new(&project_path));

    for name in ["00_renforge_patch.rpy", "00_renforge_patch.rpyc",
                 "renforge_translations.rpy", "renforge_translations.rpyc"] {
        let p = game.join(name);
        if p.exists() && std::fs::remove_file(&p).is_ok() { removed += 1; }
    }
    // встроенные шрифты RenForge
    if let Ok(rd) = std::fs::read_dir(&game) {
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if fname.starts_with("renforge_font_") && fname.ends_with(".ttf") {
                if std::fs::remove_file(e.path()).is_ok() { removed += 1; }
            }
        }
    }
    // кэш движка (перекомпилируется при запуске)
    let cache = game.join("cache");
    if cache.exists() { let _ = std::fs::remove_dir_all(&cache); }
    // сбрасываем указатель «собранной» пары
    let built = Path::new(&project_path).join(".renforge").join("built");
    if built.exists() { let _ = std::fs::remove_file(&built); }

    Ok(format!("removed:{}", removed))
}

#[tauri::command]
async fn run_unrpa(app: tauri::AppHandle, file_path: String) -> Result<String, String> {
    let parent_dir = Path::new(&file_path).parent().unwrap().to_string_lossy().to_string();
    
    let sidecar = app.shell().sidecar("unrpa").map_err(|e| e.to_string())?;
    
    // --continue-on-error: один проблемный файл в архиве (напр. зарезервированное
    // Windows-имя или коллизия путей) не должен срывать распаковку всего .rpa.
    // Без него легаси-архивы (Analogue 6.13) распаковывались лишь наполовину.
    let output = sidecar.args(["--continue-on-error", "-mp", &parent_dir, &file_path])
        .output()
        .await
        .map_err(|e| e.to_string())?;
        
    if output.status.success() { 
        // ВАЖНО: после распаковки архив нельзя оставлять рядом с распакованными файлами.
        // Ren'Py грузит модули И из .rpa, И из loose-копий → каждый init/translate-блок
        // выполняется ДВАЖДЫ. Для строковых переводов (`translate <lang> strings`) это
        // фатально: StringTranslator.add() падает на дубликате `old` ("A translation for
        // ... already exists") — практически на каждой строке (напр. ButterflySoup, везущая
        // родные tl/ на 22 языка). Отключаем архив переименованием в .renforge-disabled
        // (обратимо: можно вернуть расширение .rpa). Содержимое уже распаковано в loose.
        let disabled = format!("{}.renforge-disabled", &file_path);
        let _ = std::fs::remove_file(&disabled); // вдруг остался с прошлого прогона
        if std::fs::rename(&file_path, &disabled).is_err() {
            // не смогли переименовать (залочен?) — пробуем удалить, чтобы убрать двойную загрузку
            let _ = std::fs::remove_file(&file_path);
        }
        Ok("Распаковано".to_string()) 
    } else { 
        let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("Ошибка распаковки: {}", err_msg)) 
    }
}

#[tauri::command]
fn get_images_list(project_path: String, target_lang: String) -> Result<Vec<ImageEntry>, String> {
    let game_dir = Path::new(&project_path).join("game");
    let tl_dir = game_dir.join("tl").join(&target_lang);
    let mut images = Vec::new();

    if !game_dir.exists() {
        return Ok(images);
    }

    for entry in WalkDir::new(&game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        if path.components().any(|c| c.as_os_str() == "tl" || c.as_os_str() == "cache") {
            continue;
        }

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "webp" {
                    let rel_path = path.strip_prefix(&game_dir).unwrap_or(path).to_string_lossy().replace("\\", "/");
                    let translated_path = tl_dir.join(&rel_path);
                    
                    let is_translated = translated_path.exists();
                    let trans_path_str = if is_translated {
                        Some(translated_path.to_string_lossy().to_string())
                    } else {
                        None
                    };

                    images.push(ImageEntry {
                        original_path: path.to_string_lossy().to_string(),
                        rel_path,
                        is_translated,
                        translated_path: trans_path_str,
                    });
                }
            }
        }
    }
    Ok(images)
}

#[tauri::command]
fn import_localized_image(project_path: String, target_lang: String, rel_path: String, source_file_path: String) -> Result<String, String> {
    let game_dir = Path::new(&project_path).join("game");
    let target_path = game_dir.join("tl").join(&target_lang).join(&rel_path);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Ошибка создания папок: {}", e))?;
    }

    fs::copy(&source_file_path, &target_path).map_err(|e| format!("Ошибка копирования: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_localized_image(project_path: String, target_lang: String, rel_path: String) -> Result<(), String> {
    let game_dir = Path::new(&project_path).join("game");
    let target_path = game_dir.join("tl").join(&target_lang).join(&rel_path);

    if target_path.exists() {
        fs::remove_file(&target_path).map_err(|e| format!("Ошибка удаления: {}", e))?;
    }
    Ok(())
}

fn build_audio_mapping(game_dir: &Path) -> HashMap<String, (String, String)> {
    let mut mapping = HashMap::new();
    
    for entry in WalkDir::new(game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        if path.components().any(|c| c.as_os_str() == "tl" || c.as_os_str() == "cache") {
            continue;
        }
        
        if path.extension().and_then(|s| s.to_str()) == Some("rpy") {
            let rel_script_path = path.strip_prefix(game_dir).unwrap_or(path).to_string_lossy().replace("\\", "/");
            
            if let Ok(content) = fs::read_to_string(path) {
                let mut last_voice_filename: Option<String> = None;
                
                for line in content.lines() {
                    let trim_line = line.trim();
                    
                    if trim_line.is_empty() || trim_line.starts_with('#') {
                        continue;
                    }
                    
                    // --- Триггер озвучки: voice / play <канал> / renpy.*.play(...) ---
                    // Берём только «голосоподобные» каналы/пути, чтобы не цеплять музыку и SFX.
                    if let Some(fname) = detect_voice_trigger(trim_line) {
                        last_voice_filename = Some(fname);
                        continue;
                    }
                    
                    if last_voice_filename.is_some() {
                        if trim_line.starts_with("label ") || trim_line.starts_with("menu:") || 
                           trim_line.starts_with("return") || trim_line.starts_with("jump ") || 
                           trim_line.starts_with("call ") {
                            last_voice_filename = None;
                            continue;
                        }
                        
                        if trim_line.starts_with('$') || trim_line.starts_with("python:") || 
                           trim_line.starts_with("default ") || trim_line.starts_with("define ") ||
                           trim_line.starts_with("show ") || trim_line.starts_with("scene ") || 
                           trim_line.starts_with("play ") || trim_line.starts_with("hide ") ||
                           trim_line.starts_with("image ") || trim_line.starts_with("transform ") ||
                           trim_line.starts_with("camera ") || trim_line.starts_with("with ") ||
                           trim_line.starts_with("window ") {
                            continue;
                        }
                        
                        if trim_line.contains('"') || trim_line.contains('\'') {
                            mapping.insert(
                                last_voice_filename.take().unwrap(), 
                                (trim_line.to_string(), rel_script_path.clone())
                            );
                        }
                    }
                }
            }
        }
    }
    mapping
}

/// Извлекает первую строку в кавычках (' или ") из строки скрипта.
fn first_quoted(line: &str) -> Option<String> {
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '"' || c == '\'' {
            let mut j = i + 1;
            let mut s = String::new();
            while j < bytes.len() && bytes[j] != c {
                s.push(bytes[j]);
                j += 1;
            }
            if j < bytes.len() { return Some(s); }
            return None;
        }
        i += 1;
    }
    None
}

fn is_audio_file(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(".ogg") || n.ends_with(".mp3") || n.ends_with(".wav") || n.ends_with(".opus")
}

fn filename_of(p: &str) -> String {
    let full = p.replace('\\', "/");
    full.split('/').last().unwrap_or(&full).to_lowercase()
}

/// «Голосоподобность» по имени канала и/или пути к файлу.
fn is_voicey(channel: &str, path: &str) -> bool {
    let c = channel.to_lowercase();
    if c == "voice" || c == "vo" || c == "va" || c == "voc" || c == "cv" {
        return true;
    }
    if c.contains("voice") || c.contains("vox") || c.contains("seiyuu") || c.contains("dub") {
        return true;
    }
    let p = path.to_lowercase();
    p.contains("/voice/") || p.contains("/voices/") || p.contains("/vo/")
        || p.contains("/cv/") || p.contains("/dub/") || p.contains("seiyuu")
        || p.starts_with("voice/") || p.starts_with("vo/") || p.starts_with("voices/")
}

/// Определяет, задаёт ли строка скрипта проигрывание ГОЛОСОВОГО файла.
/// Возвращает имя аудиофайла (lowercase) для последующей привязки к реплике.
fn detect_voice_trigger(trim_line: &str) -> Option<String> {
    // voice "file"
    if trim_line.starts_with("voice ") {
        if let Some(p) = first_quoted(trim_line) {
            if is_audio_file(&p) { return Some(filename_of(&p)); }
        }
        return None;
    }
    // play <channel> "file"  (включая play voice ...)
    if trim_line.starts_with("play ") {
        let after = trim_line["play ".len()..].trim_start();
        let channel = after.split_whitespace().next().unwrap_or("").to_lowercase();
        if let Some(p) = first_quoted(trim_line) {
            if is_audio_file(&p) && is_voicey(&channel, &p) {
                return Some(filename_of(&p));
            }
        }
        return None;
    }
    // renpy.sound.play(...) / renpy.play(...) / renpy.music.play(...)
    if trim_line.contains("renpy.sound.play(") || trim_line.contains("renpy.play(")
        || trim_line.contains("renpy.music.play(") {
        if let Some(p) = first_quoted(trim_line) {
            if is_audio_file(&p) && is_voicey("", &p) {
                return Some(filename_of(&p));
            }
        }
    }
    None
}

/// Строит маппинг «аудиофайл → (текст реплики, скрипт)» из колонки prefix в БД.
/// Сайдкар при извлечении кладёт в prefix оператор озвучки (напр. voice "x.ogg"
/// или voice get_voice("a0001.ogg")). Это основной источник для .rpyc-игр,
/// где .rpy на диске нет.
fn build_audio_mapping_from_db(project_path: &str) -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    if let Ok(conn) = crate::db::get_db_conn(project_path) {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT original, file_path, prefix FROM translations WHERE prefix IS NOT NULL AND prefix != ''"
        ) {
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, String>(1).unwrap_or_default(),
                    row.get::<_, String>(2).unwrap_or_default(),
                ))
            });
            if let Ok(rows) = rows {
                for (original, file_path, prefix) in rows.flatten() {
                    if let Some(p) = first_quoted(&prefix) {
                        if is_audio_file(&p) {
                            map.entry(filename_of(&p)).or_insert((original, file_path));
                        }
                    }
                }
            }
        }
    }
    map
}

#[tauri::command]
fn get_audio_list(project_path: String, target_lang: String) -> Result<Vec<AudioEntry>, String> {
    let game_dir = Path::new(&project_path).join("game");
    let tl_dir = game_dir.join("tl").join(&target_lang);
    let mut audio = Vec::new();

    if !game_dir.exists() { return Ok(audio); }
    
    let audio_mapping_db = build_audio_mapping_from_db(&project_path);
    let audio_mapping = build_audio_mapping(&game_dir);

    for entry in WalkDir::new(&game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == "tl" || c.as_os_str() == "cache") { continue; }

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                if ext == "ogg" || ext == "mp3" || ext == "wav" {
                    let rel_path = path.strip_prefix(&game_dir).unwrap_or(path).to_string_lossy().replace("\\", "/");
                    let translated_path = tl_dir.join(&rel_path);
                    let is_translated = translated_path.exists();
                    let trans_path_str = if is_translated { Some(translated_path.to_string_lossy().to_string()) } else { None };
                    
                    let rel_path_clean = rel_path.replace("\\", "/");
                    let audio_filename = rel_path_clean.split('/').last().unwrap_or(&rel_path_clean).to_lowercase();
                    
                    let (mapped_text, mapped_script) = match audio_mapping_db.get(&audio_filename)
                        .or_else(|| audio_mapping.get(&audio_filename)) {
                        Some((t, s)) => (Some(t.clone()), Some(s.clone())),
                        None => (None, None)
                    };

                    audio.push(AudioEntry {
                        original_path: path.to_string_lossy().to_string(),
                        rel_path, is_translated, translated_path: trans_path_str,
                        mapped_text, mapped_script
                    });
                }
            }
        }
    }
    Ok(audio)
}

#[tauri::command]
fn import_localized_audio(project_path: String, target_lang: String, rel_path: String, source_file_path: String) -> Result<String, String> {
    let game_dir = Path::new(&project_path).join("game");
    let target_path = game_dir.join("tl").join(&target_lang).join(&rel_path);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Ошибка создания папок: {}", e))?;
    }
    fs::copy(&source_file_path, &target_path).map_err(|e| format!("Ошибка копирования: {}", e))?;
    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_localized_audio(project_path: String, target_lang: String, rel_path: String) -> Result<(), String> {
    let game_dir = Path::new(&project_path).join("game");
    let target_path = game_dir.join("tl").join(&target_lang).join(&rel_path);
    if target_path.exists() {
        fs::remove_file(&target_path).map_err(|e| format!("Ошибка удаления: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn open_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut win_path = path.replace("/", "\\");
        
        if win_path.starts_with("\\\\?\\") {
            win_path = win_path.replace("\\\\?\\", "");
        }

        Command::new("explorer")
            .arg("/select,")
            .arg(&win_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = Path::new(&path).parent().unwrap_or(Path::new(&path));
        Command::new("xdg-open").arg(parent).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init()) 
        .plugin(tauri_plugin_opener::init()) 
        .invoke_handler(tauri::generate_handler![
            read_text_file, write_text_file, scan_project, run_unrpa, discover_source_languages,
            prepare_writable, export_translation, llm_chat_request, is_path_writable, remove_renforge_mod, cancel_export, export_strings, list_game_files,
            generate_translations, apply_renforge_patch, get_project_fonts, get_character_mapping, get_images_list, import_localized_image, delete_localized_image, open_in_explorer,
            get_audio_list, import_localized_audio, delete_localized_audio, extract_and_ingest_project,
            migrate_translations,
            
            // Команды из модуля db
            db::search_in_db, 
            db::upsert_translations_batch, 
            db::delete_translations,
            db::get_translation_stats,
            db::get_translations_for_file,
            db::get_duplicate_originals,
            db::get_project_languages,
            db::get_project_meta,
            db::list_translation_pairs,
            db::set_active_pair,
            db::use_legacy_db,
            db::delete_translation_pair,
            tm::tm_contribute, tm::tm_fill, tm::tm_list, tm::tm_upsert, tm::tm_delete, tm::tm_count, tm::tm_clear,
            preview_generated_translations, decompile_rpyc,
            get_delivery_hooks, save_delivery_hooks, validate_delivery_hook,
            read_uncovered, clear_uncovered
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod tests {
    use super::*;

    // Защита плейсхолдеров (Этап 1): извлечение интерполяций [var] из строки.
    #[test]
    fn test_extract_interps() {
        assert_eq!(extract_interps("Привет, [name]!"), vec!["[name]".to_string()]);
        assert_eq!(extract_interps("[a] и [b]"), vec!["[a]".to_string(), "[b]".to_string()]);
        assert!(extract_interps("без переменных").is_empty());
        // {текст-теги} — не интерполяция, не захватываются
        assert!(extract_interps("{b}жирный{/b}").is_empty());
    }

    // Финальный предохранитель доставки: перевод с ЧУЖОЙ [var] должен отсеиваться.
    #[test]
    fn test_extract_interps_detects_foreign_var() {
        let orig = extract_interps("Hello [mc]");
        let trans = extract_interps("Привет [ПЕР]"); // несуществующая переменная
        let has_foreign = trans.iter().any(|t| !orig.contains(t));
        assert!(has_foreign);
    }

    // Фильтр служебных файлов RenForge при экспорте «Полная игра».
    #[test]
    fn test_is_renforge_workfile() {
        assert!(is_renforge_workfile(".renforge"));
        assert!(is_renforge_workfile(".renforge/english-russian.db"));
        assert!(is_renforge_workfile("renforge.db"));
        assert!(is_renforge_workfile("renforge.db-wal"));
        assert!(is_renforge_workfile("renforge_ast.json"));
        // обычные файлы игры — НЕ служебные (должны экспортироваться)
        assert!(!is_renforge_workfile("game/script.rpyc"));
        assert!(!is_renforge_workfile("game/renforge_translations.rpy")); // часть мода, копируется
        assert!(!is_renforge_workfile("renpy/common/00gui.rpy"));
    }

    // Экранирование для Python-литерала u"..." в рантайм-словаре доставки.
    #[test]
    fn test_escape_py_double() {
        assert_eq!(escape_py_double("a\"b"), "a\\\"b");
        assert_eq!(escape_py_double("c\\d"), "c\\\\d");
        assert_eq!(escape_py_double("e\nf"), "e\\nf");
        // U+2028/U+2029 -> \u-форма (защита от построчных лексеров, напр. Ren'Py)
        assert_eq!(escape_py_double("g\u{2028}h"), "g\\u2028h");
        assert_eq!(escape_py_double("i\u{2029}j"), "i\\u2029j");
    }
}
