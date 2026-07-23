use serde::{Deserialize, Serialize};

// --- Модели для старого функционала (оставляем) ---
#[derive(Serialize, Deserialize)]
pub struct ProjectFiles {
    pub rpa_files: Vec<String>,
    pub rpyc_files: Vec<String>,
    pub rpy_files: Vec<String>,
    pub tl_files: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct FileStats {
    pub total: i32,
    pub translated: i32,
    #[serde(default)]
    pub outdated: i32,
}

#[derive(Serialize, Deserialize)]
pub struct ImageEntry {
    pub original_path: String,
    pub rel_path: String,
    pub is_translated: bool,
    pub translated_path: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AudioEntry {
    pub original_path: String,
    pub rel_path: String,
    pub is_translated: bool,
    pub translated_path: Option<String>,
    pub mapped_text: Option<String>,    
    pub mapped_script: Option<String>,
}

// --- НОВЫЕ МОДЕЛИ: Структуры JSON от Python-экстрактора ---
#[derive(Serialize, Deserialize, Debug)]
pub struct ExtractedString {
    #[serde(rename = "type")]
    pub block_type: String,
    pub id: String,
    pub file: String,
    pub line: i32,
    pub who: Option<String>,
    pub what: String,
    pub prefix: Option<String>,
    /// Способ извлечения строки: "ast" (из .rpyc через AST) | "regex" (текстовый парс .rpy).
    #[serde(default)]
    pub source: Option<String>,
    /// Иные текстовые варианты этой же строки (по translation id) из одноязычных
    /// источников (base + tl/<same-lang>). Доставка регистрирует перевод под всеми —
    /// строка матчится независимо от того, какой текст показан в рантайме (multi-key).
    #[serde(default)]
    pub alt_texts: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtractedData {
    pub project_name: String,
    pub is_legacy_format: Option<bool>, 
    pub available_languages: Option<Vec<String>>,
    pub source_language: Option<String>,
    #[serde(default)]
    pub game_name: Option<String>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub engine_version: Option<String>,
    pub strings: Vec<ExtractedString>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FontInfo {
    pub rel_path: String,
    pub name: String,
    /// Коды покрываемых шрифтом письменностей ("latin", "cyrillic", "greek",
    /// "japanese", …). Полный перечень здесь не дублируем — источник истины:
    /// probe-таблица определения покрытия в lib.rs.
    pub scripts: Vec<String>,
}

/// Поштучная подмена: source — шрифт игры (rel_path), target — путь к целевому
/// шрифту (None = встроенный DejaVuSans движка).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FontRemap {
    pub source: String,
    pub target: Option<String>,
}

// --- ОБНОВЛЕННАЯ МОДЕЛЬ: Запись в БД SQLite ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DbEntry {
    pub id: String,
    pub block_type: String, // 'dialogue', 'menu', 'ui', 'python'
    pub file_path: String,
    pub line_number: i32,
    pub who: Option<String>,
    pub original: String,
    pub translation: String,
    pub status: String, // 'untranslated', 'translated', 'error', 'outdated'
    pub prefix: Option<String>,
    /// Прежний оригинал (для строк, перенесённых fuzzy-миграцией со статусом outdated).
    #[serde(default)]
    pub prev_original: Option<String>,
    /// Переопределение канала доставки: None/'auto' = по block_type, 'say'|'ui'|'both'.
    #[serde(default)]
    pub channel: Option<String>,

    /// Ручная отметка «перевод подтверждён». Нужна для строк, где корректный перевод
    /// совпадает с оригиналом (… , — , числа, имена) — иначе они вечно «непереведённые».
    #[serde(default)]
    pub confirmed: Option<bool>,

    /// Способ извлечения строки: "ast" | "regex" (для манульных строк — None). Диагностика
    /// надёжности: regex-извлечение грубее AST.
    #[serde(default)]
    pub source: Option<String>,

    /// Альтернативные текстовые варианты (JSON-массив строк) для multi-key delivery.
    /// Хранится как TEXT(JSON) в БД; фронт парсит для показа контекста, доставка — для
    /// регистрации доп. ключей. None/пусто — обычная строка с одним ключом.
    #[serde(default)]
    pub alt_texts: Option<String>,
}

/// Отчёт о миграции перевода между версиями игры.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct MigrationReport {
    pub carried_exact: u32,      // перенесено точно (id совпал, текст не менялся)
    pub carried_fuzzy: u32,      // перенесено с пометкой «требует проверки»
    pub new_strings: u32,        // действительно новые строки (аналога в старой версии нет)
    pub still_untranslated: u32, // были и в старой версии, но без перевода (как и раньше)
    pub old_unused: u32,         // строк старого перевода не нашли места в новой версии
}


/// Пользовательский хук доставки (экспертный режим): произвольный Python/Ren'Py-код,
/// вплетаемый в генерируемый рантайм-файл. phase: "early" | "init"; порядок — по позиции
/// в списке (внутри фазы исполняется после наших каналов, когда API/словари уже готовы).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeliveryHook {
    pub name: String,
    pub phase: String,
    pub enabled: bool,
    pub code: String,
    /// Область хранения: "global" (AppData, общий для всех проектов) | "project" (.renforge).
    /// В самих файлах не сохраняется (определяется файлом); проставляется при чтении и
    /// используется фронтом/сплитом при сохранении.
    #[serde(default)]
    pub scope: Option<String>,
}
