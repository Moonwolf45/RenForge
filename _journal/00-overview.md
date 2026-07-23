# RenForge — Обзор архитектуры

## Что это
Десктоп-инструмент для извлечения, перевода и сборки модов-локализаций визуальных новелл
на движке **Ren'Py**. Автор: **foulnike**. Лицензия GPL-3.0. Прод — v1.2.0, в работе 1.3.

## Стек
- **Оболочка:** Tauri 2.0 (Rust-бэкенд + системный WebView).
- **Фронт:** Vue 3 (`<script setup>`), Vite. Без глобального стейт-менеджера — общий
  реактивный стор на `ref`/`reactive` в `src/store.js`.
- **Бэкенд:** Rust (`src-tauri/src`), команды через `#[tauri::command]`, SQLite (rusqlite).
- **Сайдкары (PyInstaller-бинари в `src-tauri/bin/`):**
  - `rpyc_extractor` — Python-экстрактор (`tools/extractor/main.py`), вендорит unrpyc.
  - `unrpa` — распаковщик .rpa-архивов (`tools/unrpa`).
- **i18n:** `src/locales.js` — 6 языков (ru/en/zh/ja/es/pt), функция `t()`.

## Раскладка репозитория (`renforge/`)
```
src/                     фронт (Vue)
  store.js               общий реактивный стор + утилиты (scrollToBlock, showMsg…)
  actions.js             оркестрация действий фронта (extract/open/save/export…)
  diagnostics.js         реестр диагностик строк + автофиксы
  locales.js             i18n, 6 языков
  App.vue, main.js       корень приложения
  components/*.vue        UI (Editor, Dashboard, Header, галереи, модалки…)
  assets/style.css        глобальные стили + тема (var(--*))
src-tauri/
  src/
    lib.rs               ядро: почти все команды, оркестрация, доставка, экспорт
    db.rs                слой SQLite: пары языков, upsert, поиск, статистика
    models.rs            структуры (ExtractedString, DbEntry, …)
    tm.rs                Translation Memory (глобальная память переводов)
    error.rs             AppError
    main.rs              точка входа (вызывает lib::run)
  tools/
    extractor/main.py    экстрактор строк (AST + regex-фоллбэк + engine common)
    extractor/unpickler.py  легаси-загрузчик .rpyc AST (RenpyUnpickler, latin-1)
    unrpyc/              вендоренный unrpyc (для читалки/декомпиляции)
    unrpa/               вендоренный unrpa (распаковка .rpa)
  bin/                   собранные сайдкары (*-x86_64-pc-windows-msvc.exe)
```

## Сквозные потоки данных

### 1. Извлечение (Extraction)
```
Игра (game/*.rpa, *.rpyc, *.rpy)
  → [unrpa] распаковать .rpa в loose, архив → *.renforge-disabled
  → [rpyc_extractor --dir game/ --out out.json --source-lang …]
       AST по .rpyc (unpickler.explore_ast)  → source="ast"
       ИЛИ regex по .rpy (parse_rpy_file)     → source="regex"  (фоллбэк, если нет .rpyc)
       + engine common из renpy/common/*.rpy  → source="regex"  (только source=original)
  → [Rust ingest_extracted_json] фильтр мусора → INSERT в translations (БД активной пары)
       + таблица characters (define-персонажи)
```
Единица данных — «строка» (`ExtractedString` → `DbEntry`): id, block_type
(dialogue/menu/ui/python), file, line, who, original, translation, status, prefix, source…

### 2. Перевод (в редакторе / AI / TM)
Фронт читает строки файла (`get_translations_for_file`), правит `translation`, сохраняет
(`upsert_translations_batch`). Диагностики (`diagnostics.js`) считают ошибки/предупреждения.
TM (`tm.rs`) переносит совпадающие переводы между проектами/версиями.

### 3. Доставка (Delivery / Build)
```
Строки из БД (status=translated)
  → [build_runtime_rpy] генерирует renforge_translations.rpy (+ 00_renforge_patch.rpy)
       каналы say/ui/both; пропуск идентичных пар; шрифты; хуки доставки
  → кладётся в game/ ; runtime-БД renforge.db ; шрифты renforge_font_*.ttf
  → указатель .renforge/built = собранная пара
```
Экспорт «Полная игра» копирует game/ минус служебные файлы (`is_renforge_workfile`).
Экспорт строк — CSV/JSON/PO пофайлово (`export_strings`).

## Модель хранения (кратко; детали в 05)
- Одна SQLite-БД **на пару языков**: `.renforge/<source>-<target>.db`.
- Указатели-файлы: `.renforge/active` (активная пара), `.renforge/built` (собранная).
- Таблицы: `translations`, `characters`, `project_meta` (key/value).
- Кэш декомпиляции читалки: `.renforge/decomp/*.rpy`.

## Словарь терминов
- **Пара (pair)** — направление перевода source→target, своя БД.
- **Канал (channel)** — куда доставлять строку: `say` (диалоги/`translate strings`),
  `ui` (интерфейс), `both`. Авто — по block_type.
- **Prefix** — ведущий оператор строки (напр. `voice "x.ogg"`); источник маппинга аудио.
- **who** — говорящий/тег роли (`[ВЫБОР]`, `[ИНТЕРФЕЙС]`, `[ENGINE]`, `[DEFINE: код]`).
- **source** — способ извлечения строки: `ast` | `regex`.
- **confirmed** — ручная отметка «перевод верен» (когда перевод == оригиналу).
- **legacy** — старый формат Ren'Py (иная структура AST/пиклов), детектится при загрузке.
- **loose .rpy/.rpyc** — файлы рядом с архивом (не внутри .rpa).

## Указатель на детальные разделы
`01` бэкенд · `02` экстрактор · `03` фронт-ядро · `04` компоненты · `05` БД ·
`06` решения · `07` worklog · `08` краевые случаи (эмпирика движков).
