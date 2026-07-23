# Модель данных и БД

SQLite (rusqlite), WAL, synchronous NORMAL. Одна БД **на пару языков**.

## Расположение и указатели (в `<project>/.renforge/`)
- **`<source>-<target>.db`** — БД конкретной пары (напр. `english-russian.db`). Имя пары —
  `pair_name()` (санитизация токенов языка).
- **`active`** — текстовый указатель на активную пару (имя без `.db`). Нет файла → legacy.
- **`built`** — имя пары, чей мод сейчас материализован в `game/`. Нет → ничего не собрано.
- **`decomp/*.rpy`** — кэш декомпиляции для читалки.
- **`delivery_hooks.json`** — проектные хуки доставки.
- **Legacy:** `<project>/renforge.db` (старые проекты до разделения по парам). Активна, если
  указателя `active` нет.

## Таблица `translations`
| Колонка | Тип | Смысл |
|---------|-----|-------|
| `id` | TEXT PK | id строки (из AST translation_identifier / `<label>_<md5>` / `manual_<djb2>` / `engine_*`) |
| `block_type` | TEXT | dialogue / menu / ui / python |
| `file_path` | TEXT | логическое имя исходника (`.rpy`, из AST) или `__renforge_manual__` / `engine (renpy common)` |
| `line_number` | INTEGER | номер строки (навигация; у ручных/движковых косметический) |
| `who` | TEXT | говорящий / тег роли (`[ВЫБОР]`, `[ИНТЕРФЕЙС]`, `[ENGINE]`, `[DEFINE: код]`) |
| `original` | TEXT | оригинал |
| `translation` | TEXT | перевод (пусто = не переведено) |
| `status` | TEXT | untranslated / translated / error / outdated |
| `prefix` | TEXT | ведущий оператор строки (напр. `voice "x.ogg"`) — источник маппинга аудио |
| `prev_original` | TEXT | прежний оригинал (для fuzzy-перенесённых при миграции версий) |
| `channel` | TEXT | override канала доставки: NULL/auto = по block_type, `say`/`ui`/`both` |
| `confirmed` | INTEGER | ручная отметка «перевод верен» (когда перевод == оригиналу) |
| `source` | TEXT | способ извлечения: `ast` / `regex` (у ручных — NULL) |
| `alt_texts` | TEXT | JSON-массив иных текстовых вариантов строки (multi-key delivery); NULL/пусто = обычная строка |

Индекс `idx_translations_file` по `file_path` (иначе открытие файла и статистика — полный
скан; «долгое открытие» на 100k+ строк).

**Миграции:** все новые колонки добавлены идемпотентными `ALTER TABLE ADD COLUMN` (дубль-
ошибка игнорируется), порядок исторический: prefix → prev_original → channel → confirmed →
source → alt_texts. Схема-изменения строго идемпотентны (была жалоба на битую БД с 1.1).

## Таблица `characters`
`code TEXT PK, name TEXT, file_path TEXT, line_number INTEGER`. Маппинг код→имя персонажа
(из define Character). Создаётся в get_db_conn; **наполняется** в `lib.rs::ingest_extracted_json`
(строки block_type=python с who=`[DEFINE: код]`), очищается перед каждым ingest. Читается
`get_character_mapping`.

## Таблица `project_meta` (key/value)
Ключи: `available_languages` (JSON), `source_language`, `target_language`, `built_dirty`
(`1` = БД менялась после сборки мода → собранный мод устарел; сбрасывается в `0` при
`mark_pair_built`).

## Статусы строки (вычисление на фронте — `diagnostics.js::blockStatus`)
Приоритет: **error** (любая error-диагностика — missing-tag/extra-interp — блокирует
сохранение) > **translated** (если `confirmed` && непустой перевод — бьёт правило
«перевод==оригинал», но НЕ ошибки) > **untranslated** (пусто или перевод==оригиналу без
confirmed) > **outdated** (есть prev_original) > **translated**. В БД `status` пишется при
сохранении (`saveFile` проставляет из getBlockStatus).

## TM (отдельная глобальная БД)
`tm.db` в app_data_dir (не в проекте): таблица `tm` c PK `(target_lang, original)`,
translation, hits. Кросс-проектная память переводов. Наполняется `tm_contribute` (фоном),
заливается `tm_fill` (точные совпадения → непереведённые «к проверке»).

## Поток «строка» сквозь систему
```
экстрактор JSON (ExtractedString: type/id/file/line/who/what/prefix/source)
  → ingest_extracted_json (фильтр мусора, дедуп id)
  → translations (translation='', status='untranslated')
  → редактор правит translation → saveFile → upsert_translations_batch (status пересчитан)
  → build_runtime_rpy читает status IN ('translated','outdated') → renforge_translations.rpy
```
