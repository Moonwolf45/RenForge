# Бэкенд (Rust, src-tauri/src)

Модули: `lib.rs` (ядро), `db.rs`, `models.rs`, `tm.rs`, `error.rs`, `main.rs`. Все
`#[tauri::command]` регистрируются в `run()` через `generate_handler!`.

**Сквозной принцип доставки:** стратегия перевода намеренно **языконезависима** — движок-
язык НЕ переключается (`change_language` отравляет persistent и роняет игры). Перевод
доставляется в рантайме без translate-блоков, через прямую инъекцию в словари движка.

## lib.rs — карта функций

### Извлечение / оркестрация
- **`scan_project(path, target_lang) -> ProjectFiles` (command)** — обход `game/` (walkdir)
  в списки .rpa/.rpyc/.rpy/tl. tl определяется вхождением `/tl/<lang>/` (unix и windows);
  .rpyc из tl отбрасываются (оверлей). В spawn_blocking.
- **`extract_and_ingest_project(project_path, source_lang?, target_lang?) -> i64` (command)**
  — запускает сайдкар (`--dir --out --source-lang`), затем ingest. `source_lang` def "auto"
  резолвится в конкретный язык; **активная пара ставится через `db::set_active_pair` ДО
  ingest** (данные лягут в нужную БД). После — пишет target_language в project_meta.
- **`discover_source_languages(project_path) -> Vec<String>` (command)** — быстрое
  определение языков-источников БЕЗ полного извлечения (решает «курицу и яйцо» для селектора
  «Переводить с»). Экстрактор `--list-languages`, парсит строку `RENFORGE_LANGS:`.
- **`ingest_extracted_json(project_path, out_json) -> String`** — грузит JSON в БД: фильтр
  мусора (пустые, чистые `[var]`, одиночные символы кроме `HSV`, чистые числа), таблицы
  translations+characters, движковые supplement-строки (хардкод, дедуп WHERE NOT EXISTS),
  метаданные. Дедуп id суффиксом `_N`. **Re-extract:** `ON CONFLICT(id) DO UPDATE` рефрешит только структурные поля + original/prefix/source/alt_texts; translation/status/channel/confirmed/prev_original НЕ трогает → перевод строк с неизменным id переживает переизвлечение. По окончании удаляет out_json. Вынесено из команды
  для CLI/тестов без AppHandle.

### Распаковка .rpa
- **`run_unrpa(file_path) -> String` (command)** — сайдкар unrpa (`--continue-on-error -mp`),
  затем **отключает архив** переименованием в `*.renforge-disabled`. Причина: Ren'Py грузит
  И из .rpa, И из loose → двойное выполнение init/translate → `StringTranslator.add()` падает
  на дубликате. `--continue-on-error`: битый файл не срывает всю распаковку (легаси Analogue).
  Переименование обратимо; если залочен — пытается удалить.

### Доставка / сборка runtime .rpy
Константы-шаблоны Python: `RENFORGE_EARLY_HOOK`, `RENFORGE_RUNTIME`, `RENFORGE_HOOK_API_EARLY`,
`RENFORGE_HOOK_API_INIT` (плейсхолдер `{LANG}`), `RENFORGE_LOG_NOOP` / `RENFORGE_LOG_DIAG`
(диагностика покрытия, opt-in — см. ниже).
- **`build_runtime_rpy(project_path, target_lang, diagnostic) -> (String, BuildCounts)`** —
  строит содержимое рантайм-файла (общий код для генерации и превью). Доставляет строки со
  `status IN ('translated','outdated')` и непустым переводом (**outdated = перенос/TM: с 1.3
  доставляются**, чтобы их было видно в игре для ревью; `BuildCounts.review` = сколько
  доставленных оригиналов имели status='outdated'). `BuildCounts {say, ui, review, skipped_bad}`
  (serde) уходит на фронт для локализованного отчёта. **Multi-key:** кроме `original`, тот же перевод
  регистрируется под каждым `alt_texts` (JSON-массив вариантов из одноязычных источников) —
  с предохранителем KeyError (alt-ключ добавляется, только если содержит все `[var]`
  перевода). Рантайм матчит любой показанный вариант. **Каналы say/ui/both:** колонка `channel`
  переопределяет авто (dialogue|menu→say через `config.say_menu_text_filter`; иначе→ui прямой
  инъекцией в `translator.strings[lang]`). **Пропуск идентичных пар** (original==translation):
  доставлять нечего. **Предохранитель чужих `[var]`** (`extract_interps`): интерполяция в
  переводе, которой нет в оригинале → пропуск (риск KeyError), счётчик skipped_bad. **Почему
  `python early`:** словари + ранний хук `_()` ставятся ДО init, т.к. `image` с `Text(_())`
  «запекают» `_()` на init. Хуки: early + init 1000.
  - Каналы в RENFORGE_RUNTIME: К1 диалоги/меню (say_menu_text_filter, цепочка), К2 UI (прямая
    запись в `stl.translations[o]`, минует add() и его проверку коллизий), К3 промпты
    `renpy.input` (обёртка до подстановки `[var]`), К5 легаси SL1-UI (обёртка `renpy.ui.text`
    для 6.12–6.17), К6 ранний хук `_()`/`translate_string` + `store._/__`.
  - **Диагностика покрытия (opt-in, roadmap 0.1):** `build_runtime_rpy` берёт флаг `diagnostic`.
    После словарей в `python early` ВСЕГДА определяется хелпер `_renforge_log_uncovered(chan, s)` —
    реальный логгер при `diagnostic=true` (`RENFORGE_LOG_DIAG`), иначе no-op (`RENFORGE_LOG_NOOP`).
    Шаблоны зовут его на ПРОМАХАХ каналов: К6 `translate_string` логирует `ui` только если игра
    сама не перевела (`_renforge_ts_orig(s)==s`), К1 say-lookup → `say`, К3 input → `input`, К5
    ui.text → `uitext`. Реальный логгер дедуплицирует (set по `(chan,s)` за сессию), экранирует
    `\\`/`\n`/`\r`/`\t`, дописывает `chan\t<escaped>\n` (utf-8) в `config.basedir/renforge_uncovered.log`;
    всё в try/except (в игре не падает). Флаг протянут: `generate_translations`(command) →
    `generate_translations_core` → CLI (2 сайта, `false`); превью строит без диагностики (`false`).
- **`generate_translations_core` / `generate_translations` (command)** — строит рантайм,
  чистит legacy-артефакты прошлых версий RenForge, пишет `game/renforge_translations.rpy`.
  Возвращают `BuildCounts` (не строку) — фронт собирает локализованный отчёт сборки.
- **`preview_generated_translations(...) -> String` (command)** — если мод собран, отдаёт
  реальный файл, иначе строит превью.
- **`read_uncovered(project_path) -> Vec<UncoveredEntry>` (command)** — диагностика покрытия:
  читает `renforge_uncovered.log` из корня проекта (пишет рантайм-логгер, см. build_runtime_rpy),
  разэкранирует (`unescape_log`), дедупит и сверяет с БД. `UncoveredEntry{chan,text,in_db,translated}`
  (serde): `SELECT original ... GROUP BY original` → `in_db` + есть ли непустой перевод. `in_db=false`
  = кандидаты (видно в игре, но извлечение не поймало). Нет файла → пустой список.
- **`clear_uncovered(project_path) (command)`** — удаляет лог непокрытого (сброс перед прогоном).
- **`apply_renforge_patch_core / apply_renforge_patch(...) (command)`** — генерирует
  `game/00_renforge_patch.rpy` (`init -999`): dev/console + **санация «ядовитого»
  `_preferences.language`** (если язык неизвестен движку — сброс на None, самолечение
  отравленных сейвов). **Поштучный ремап шрифтов** на рендере: каждый source→свой target
  (или встроенный DejaVuSans), копии дедуплицируются (`renforge_font_N.ttf`), маппинг в
  `config.font_replacement_map`. Снимает read-only, чистит game/cache и старые tl/.rpyc,
  помечает пару собранной (`db::mark_pair_built`).
- **`remove_renforge_mod(...) -> String` (command)** — удаляет доставку (патч, рантайм,
  renforge_font_*, cache, флаг built). НЕ трогает БД и tl/<target> медиа.
- **`decompile_rpyc(project_path, file_path) -> String` (command)** — read-only декомпиляция
  через сайдкар, кэш в `.renforge/decomp/<stem>.rpy`. Если рядом loose .rpy — отдаёт его.
  Не пишет рядом с оригиналом (иначе двойная загрузка). Устойчивость к легаси-пиклам —
  на стороне сайдкара unrpyc (latin-1, см. 02).

### Экспорт строк (перевод как файлы)
- **`export_strings_core / export_strings(...) (command)` -> StringsExportResult** — пакетный
  экспорт строк активной пары **пофайлово** во все три формата (CSV/JSON/PO): по файлу-
  исходнику на каждый (`script.rpy → script.rpy.po`), группировка по file_path, сортировка
  по line. Несёт `id` как ключ (msgctxt/колонка) — задел под будущий пакетный импорт по
  именам. Пустой путь → группа `_unknown`.
- **`build_po`** — gettext: заголовок, `#: file:line`, `#. who:`, `#, fuzzy` для outdated,
  `msgctxt`(id)/`msgid`/`msgstr`.
- **`build_csv`** — `ID;Original;Translation`, `\n`→`[BR]`, дубль кавычек, **анти-инъекция
  формул** (префикс `'` если начинается с `= + - @`).
- **`build_json`** — массив `{id, original, translation}`.
- **`write_export_file`** — безопасная склейка пути; отбрасывает `.`/`..`/абсолютные (path
  traversal).
- **`po_escape` / `escape_py_double`** — экранирование для PO и Python-литерала `u"..."`.

### Экспорт дистрибутива (полная игра / мод)
- **`export_translation(...) (command)` -> ExportResult** — `mode`: "full" (вся игра с
  модом) | "mod" (оверлей). Шлёт события `export_progress {done,total}`. spawn_blocking.
- **`export_translation_core`** — проверка непустой папки: без overwrite → код "exists"
  (фронт спросит), с overwrite → очистка. Диспатч в export_full/export_mod.
- **`export_full`** — копия всей игры минус служебные. Считает размер+число (прогресс +
  проверка места, +5% запас, `fs2::available_space`) → "nospace" при нехватке. Проверяет
  флаг EXPORT_CANCEL (при отмене удаляет недокопию → "cancelled"). Залоченный/открытый файл
  (запущенная игра) пропускается → `skipped`. Прогресс не чаще ~200мс/64 файла.
- **`export_mod`** — оверлей RenForge + README о версии. Требует собранного патча. Копирует
  патч+рантайм (+.rpyc), renforge_font_*, tl/<target>/.
- **`cancel_export() (command)`** — ставит атомик EXPORT_CANCEL.
- **`is_renforge_workfile(rel) -> bool`** — служебные, не в дистрибутив: `.renforge/`,
  `renforge.db`, `renforge_ast.json`, `renforge_native.json`, `*.renforge-disabled`,
  `*.renforge_write_test.tmp`. ВАЖНО: `renforge_translations.rpy` тут НЕ служебный (часть
  мода). Фильтр модалки файлов — отдельный (см. list_game_files).
- **`copy_file_mkdir`** — копирование с созданием папок.

### Файлы и медиа
- **`list_game_files(project_path) -> Vec<GameFileInfo>` (command)** — все .rpyc/.rpy со
  статусом: `extracted` (есть строки в БД), `lang` (чужая локализация по суффиксу), `empty`.
  **Фильтр собственных файлов:** пропускает `tl/`, `renforge_*`, `00_renforge_patch.rpy/.rpyc`.
  Дедуп пары .rpyc+.rpy (ключ → .rpy).
- **`detect_lang_suffix(stem)`** — языковой суффикс (`script_ru→RU`); длинные (pt-br, zh-hant)
  первыми. Синхронизирован с экстрактором.
- **`get_images_list / import_localized_image / delete_localized_image` (command)** —
  картинки png/jpg/jpeg/webp; локализация в `tl/<lang>/`. Пропуск tl/cache.
- **`get_audio_list / import_localized_audio / delete_localized_audio` (command)** — аудио
  ogg/mp3/wav; привязка к тексту реплики (сначала из БД `build_audio_mapping_from_db`, затем
  фоллбэк-парсинг .rpy `build_audio_mapping`).
- **`build_audio_mapping`** — парс .rpy, связь голосового файла со следующей репликой;
  сброс на label/menu/return/jump/call.
- **`build_audio_mapping_from_db`** — из колонки `prefix` (сайдкар кладёт `voice "x.ogg"`).
  Основной источник для .rpyc-игр.
- **`detect_voice_trigger` / `first_quoted` / `is_audio_file` / `filename_of` / `is_voicey`**
  — определение голосового триггера, отсев музыки/SFX по имени канала/пути.
- **`get_project_fonts / list_game_fonts` (command)** — шрифты игры (.ttf/.otf) с покрытыми
  письменностями (для поштучной подмены). Пропуск renforge_font*.
- **`font_scripts(path)`** — через `ttf_parser` определяет покрытие письменностей по пробным
  глифам (покрыт только если ВСЕ глифы есть). Фактически пробует **26 письменностей**
  (latin, vietnamese, cyrillic, greek, armenian, georgian, hebrew, arabic, индийские
  devanagari/bengali/…/malayalam/sinhala, thai/lao/tibetan/myanmar/khmer/ethiopic, CJK
  japanese/chinese/korean); doc-комментарий `FontInfo` в models.rs перечисляет лишь 9 —
  устарел, покрытие шире.

### Миграция версий
- **`migrate_translations_core(new, old) -> MigrationReport`** — перенос перевода из старой
  версии. 4 ступени на каждую непереведённую новую строку: (1) точный id → carried_exact;
  (2) id был без перевода → still_untranslated; (3) fuzzy по переведённым того же файла
  (`strsim::normalized_levenshtein`, порог 0.7, лучший непользованный) → outdated +
  prev_original (carried_fuzzy), кандидаты помечаются used; (4) похожа на старый оригинал →
  still_untranslated, иначе new_strings. Одна транзакция.
- **`migrate_translations(...) (command)`** — обёртка.
- **`get_character_mapping(...) -> HashMap` (command)** — коды→имена персонажей: из таблицы
  characters, фоллбэк — парс `define ... Character(...)`. Имена чистятся `strip_renpy_tags`.
- **`strip_renpy_tags`** — удаляет `{...}`.

### LLM
- **`llm_chat_request(base_url, api_key, model, system, user, temperature) -> String`
  (command)** — OpenAI-совместимый `chat/completions` через reqwest. Идёт через Rust, чтобы
  **обойти CORS вебвью** и не светить ключ. `/chat/completions` дописывается автоматически.
  Пустой ключ → без Authorization (локальные эндпоинты). Таймаут 180с. Разбирает
  error.message; достаёт choices[0].message.content.

### Хуки доставки (экспертный режим)
- **`get_delivery_hooks / save_delivery_hooks / validate_delivery_hook` (command)** — чтение
  (глобальные + проектные), сохранение (global → %APPDATA%/RenForge, project → .renforge;
  `scope` в файл не пишется). save помечает built_dirty=1. validate — синтаксис через сайдкар
  `--check-syntax`.
- **`load_delivery_hooks / read_hooks_file`** — грузит global затем project, проставляет scope.
- **`weave_hooks(out, hooks, phase)`** — вплетает хуки фазы с реиндентом и try/except-песочницей.
  Табы→4 пробела ДО реиндента (иначе TabError в игре).
- **`indent_block` / `global_hooks_path` / `project_hooks_path`** — вспомогательные.
- API-шаблоны хуков: `RENFORGE_HOOK_API_EARLY` (renforge_tr, renforge_add — в `python early`),
  `RENFORGE_HOOK_API_INIT` (renforge_wrap, renforge_wrap_ret, renforge_filter, _rf_resolve —
  в `init 1000`).

### Файлы, права, прочее
- **`read_text_file / write_text_file` (command)** — белый список `.csv/.json/.po/.pot`
  (иначе Access denied) — защита ФС от вебвью.
- **`prepare_writable / clear_readonly_recursive / is_path_writable` (command)** — снятие
  read-only (ломает запись на всех шагах); проба записи (типичный false — Program Files/Steam
  под UAC → фронт советует скопировать игру/запуск от админа).
- **`open_in_explorer` (command)** — проводник с выделением (win `/select,`, mac `open -R`,
  linux `xdg-open`); на win снимает `\\?\`.
- **`extract_interps(s)`** — извлекает `[var]` (для предохранителя доставки); `{теги}` не
  захватывает.
- **`run()`** — точка входа Tauri: плагины (shell/dialog/opener) + регистрация всех команд
  (включая модули db и tm).
- **Юнит-тесты (`#[cfg(test)] mod tests` в хвосте lib.rs)** — `test_extract_interps`,
  `test_extract_interps_detects_foreign_var` (предохранитель чужих `[var]`),
  `test_is_renforge_workfile`, `test_escape_py_double`. Запуск: `cargo test`. Это ЕДИНСТВЕННЫЕ
  инлайн-тесты в кодовой базе; питоновские прогоны `qa.py`/`bench.py`/`e2e.py` живут отдельно
  в `_testbench/` (см. 08).

## db.rs — слой SQLite

Подробная схема — в `05-data-model-db.md`. Здесь — функции.

- **`renforge_dir / active_db_path / pair_name / sanitize_token`** — пути: БД активной пары
  `.renforge/<pair>.db` по указателю `.renforge/active`; legacy-фоллбэк `project/renforge.db`.
- **`get_db_conn(project_path) -> Connection`** — открывает БД активной пары; WAL +
  synchronous NORMAL; CREATE TABLE IF NOT EXISTS для translations/characters; **идемпотентные
  ALTER ADD COLUMN** (prefix, prev_original, channel, confirmed, source, alt_texts — дубль-ошибка
  игнорируется); индекс `idx_translations_file` (открытие файла и статистика шли полным
  сканом — «долгое открытие» на 100k+ строк).
- **`search_in_db(...) -> Vec<DbEntry>` (command)** — LIKE по original/translation, LIMIT 100.
  channel/confirmed отдаёт None (не нужны в поиске), source читает.
- **`get_translation_stats(...) -> HashMap<file, FileStats>` (command)** — COUNT/translated/
  outdated по file_path.
- **`upsert_translations_batch(project_path, entries) (command)`** — INSERT OR REPLACE (все
  14 колонок вкл. source и alt_texts). **Дедупликация:** переведённые строки распространяются на дубли с
  тем же original в других файлах (UPDATE непереведённых). Ставит built_dirty=1.
- **`delete_translations(project_path, ids) (command)`** — DELETE по id + built_dirty=1.
- **`get_translations_for_file(...) -> Vec<DbEntry>` (command)** — все строки файла по
  line_number (со всеми колонками вкл. source).
- **`get_duplicate_originals(...) -> HashMap<String, DupStat>` (command)** — `GROUP BY original
  HAVING count>1`: для каждого повторяющегося оригинала `{count, variants}` (variants = число
  РАЗНЫХ непустых переводов). Редактор помечает дубли; variants>1 = конфликт доставки (#3).
  `DupStat` сериализуется на фронт (`dupMap`).
- **`get_project_languages / get_project_meta` (command)** — чтение project_meta (key/value).
- **PairInfo / read_pair_info / mark_pair_built** — сводка пары (source/target/total/
  translated/is_active/is_legacy/is_built/is_dirty); `mark_pair_built` пишет `.renforge/built`
  = активная пара и сбрасывает built_dirty=0.
- **`list_translation_pairs(...) (command)`** — все `.renforge/*.db` + legacy renforge.db с
  total>0; активные сверху.
- **`set_active_pair / use_legacy_db / delete_translation_pair` (command)** — переключение
  пары (пишет `.renforge/active`), сброс на legacy, удаление БД пары (+WAL/SHM, сброс
  указателей active/built).

## models.rs — структуры

- **`ProjectFiles`** — rpa_files/rpyc_files/rpy_files/tl_files.
- **`FileStats`** — total/translated/outdated.
- **`ImageEntry` / `AudioEntry`** — original_path/rel_path/is_translated/translated_path
  (+ AudioEntry: mapped_text/mapped_script).
- **`ExtractedString`** — от экстрактора: block_type(type)/id/file/line/who/what/prefix/
  **source**/**alt_texts** (`Vec<String>`, serde default — варианты для multi-key).
- **`ExtractedData`** — project_name/is_legacy_format/available_languages/source_language/
  game_name/game_version/engine_version/strings.
- **`FontInfo`** — rel_path/name/scripts. **`FontRemap`** — source/target(Option).
- **`DbEntry`** — id/block_type/file_path/line_number/who/original/translation/status/prefix/
  prev_original/channel/**confirmed**/**source**/**alt_texts** (последние — serde default;
  alt_texts — Option<String> с JSON-массивом для multi-key).
- **`MigrationReport`** — carried_exact/carried_fuzzy/new_strings/still_untranslated/old_unused.
- **`DeliveryHook`** — name/phase/enabled/code/scope(Option — определяется файлом, не пишется).

## tm.rs — Translation Memory (глобальная память переводов)

Глобальная (кросс-проектная) память: `tm.db` в app_data_dir, таблица `tm` c PK
`(target_lang, original)`, столбцы translation/source_lang/hits/updated_at (+ индекс `idx_tm_target`).
- **`tm_contribute(project_path) (command)`** — заливает переведённые строки активной пары в
  TM (фоном после saveFile).
- **`tm_fill(project_path) -> i64` (command)** — точные совпадения original из TM →
  непереведённые строки активной пары («к проверке»): ставит status='outdated' +
  prev_original=original (переиспользует очередь проверки; ложного диффа «Было» нет, т.к.
  prev==original).
- **`tm_list(query, limit, offset) / tm_upsert / tm_delete / tm_count / tm_clear` (command)**
  — CRUD/просмотр для TmModal.

## error.rs — `AppError` (Io / Db / Custom) с ручным Serialize в строку (для проброса в фронт).

## main.rs — тонкая точка входа: `windows_subsystem`, вызывает `renforge_lib::run()`.
