// Headless-CLI поверх реального бэкенда RenForge.
// Позволяет прогнать полный продуктовый путь без GUI:
//   экстрактор (сайдкар) -> ingest в renforge.db -> перевод -> generate -> patch.
// Это «симуляция работы пользователя с прогой» для автоматизированных тестов.
//
// Использование:
//   renforge_cli extract  <project_dir>
//   renforge_cli set      <project_dir> <ru.json>     (UPDATE по original -> translated)
//   renforge_cli generate <project_dir> <lang>
//   renforge_cli patch    <project_dir> <lang>
//   renforge_cli stats    <project_dir>
//   renforge_cli full     <project_dir> <lang> <ru.json>

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use renforge_lib::{apply_renforge_patch_core, generate_translations_core, ingest_extracted_json, list_game_fonts, migrate_translations_core};
use renforge_lib::models::FontRemap;
use renforge_lib::db::{get_db_conn, get_translation_stats};

const SIDECAR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../bin/rpyc_extractor-x86_64-pc-windows-msvc.exe"
);

fn extract(project: &str) -> Result<String, String> {
    let game = Path::new(project).join("game");
    let out = Path::new(project).join("renforge_ast.json");
    if !game.exists() {
        return Err(format!("game dir not found: {}", game.display()));
    }
    let status = Command::new(SIDECAR)
        .arg("--dir").arg(&game)
        .arg("--out").arg(&out)
        .arg("--source-lang").arg("auto")
        .status()
        .map_err(|e| format!("не удалось запустить сайдкар: {}", e))?;
    if !status.success() {
        return Err("сайдкар завершился с ошибкой".to_string());
    }
    ingest_extracted_json(project, &out)
}

fn set_translations(project: &str, ru_json: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(ru_json).map_err(|e| e.to_string())?;
    let map: HashMap<String, String> =
        serde_json::from_str(&content).map_err(|e| format!("ошибка JSON: {}", e))?;
    let conn = get_db_conn(project).map_err(|e| e.to_string())?;
    let mut updated = 0usize;
    for (orig, tr) in &map {
        if tr.is_empty() {
            continue;
        }
        let n = conn
            .execute(
                "UPDATE translations SET translation=?1, status='translated' WHERE original=?2",
                rusqlite::params![tr, orig],
            )
            .map_err(|e| e.to_string())?;
        updated += n;
    }
    Ok(format!("обновлено строк: {}", updated))
}

fn patch(project: &str, lang: &str) -> Result<(), String> {
    // Ремапим все шрифты игры без кириллицы на встроенный DejaVu (target=None),
    // как авто-дефолт продукта.
    let game_dir = Path::new(project).join("game");
    let font_remaps: Vec<FontRemap> = list_game_fonts(&game_dir)
        .into_iter()
        .filter(|f| !f.scripts.iter().any(|s| s == "cyrillic"))
        .map(|f| FontRemap { source: f.rel_path, target: None })
        .collect();
    apply_renforge_patch_core(project.to_string(), lang.to_string(), font_remaps)
}

fn stats(project: &str) -> Result<String, String> {
    let s = get_translation_stats(project.to_string()).map_err(|e| e.to_string())?;
    let mut total = 0i32;
    let mut translated = 0i32;
    for (_f, st) in &s {
        total += st.total;
        translated += st.translated;
    }
    Ok(format!("файлов: {}, строк всего: {}, переведено: {}", s.len(), total, translated))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: renforge_cli <extract|set|generate|patch|stats|full> <project> [...]");
        std::process::exit(2);
    }
    let cmd = args[1].as_str();
    let project = args[2].as_str();

    let result: Result<String, String> = match cmd {
        "extract" => extract(project),
        "set" => set_translations(project, args.get(3).map(|s| s.as_str()).unwrap_or("")),
        "generate" => generate_translations_core(
            project.to_string(),
            args.get(3).cloned().unwrap_or_else(|| "russian".to_string()),
            false,
        ).map(|c| format!("say={} ui={} review={} skipped_bad={}", c.say, c.ui, c.review, c.skipped_bad)),
        "patch" => patch(project, args.get(3).map(|s| s.as_str()).unwrap_or("russian")).map(|_| "патч применён".to_string()),
        "migrate" => {
            let old = args.get(3).map(|s| s.as_str()).unwrap_or("");
            migrate_translations_core(project.to_string(), old.to_string())
                .map(|r| format!("exact={} fuzzy={} new={} still_untr={} gone={}",
                    r.carried_exact, r.carried_fuzzy, r.new_strings, r.still_untranslated, r.old_unused))
        }
        "stats" => stats(project),
        "full" => {
            let lang = args.get(3).cloned().unwrap_or_else(|| "russian".to_string());
            let ru = args.get(4).map(|s| s.as_str()).unwrap_or("");
            (|| {
                let e = extract(project)?;
                println!("[extract] {}", e);
                if !ru.is_empty() {
                    let s = set_translations(project, ru)?;
                    println!("[set] {}", s);
                }
                let g = generate_translations_core(project.to_string(), lang.clone(), false)?;
                println!("[generate] say={} ui={} review={} skipped_bad={}", g.say, g.ui, g.review, g.skipped_bad);
                patch(project, &lang)?;
                println!("[patch] ok");
                stats(project)
            })()
        }
        _ => Err(format!("неизвестная команда: {}", cmd)),
    };

    match result {
        Ok(msg) => println!("OK: {}", msg),
        Err(e) => {
            eprintln!("ERR: {}", e);
            std::process::exit(1);
        }
    }
}
