# Worklog

Хронология работ. Актуальный статус: **v1.2.0 в проде, 1.3.0 готовится к релизу (dev).**
Версия уже забампана и закоммичена (README/Cargo.toml×2/tauri.conf.json/package.json/Header.vue/
AboutModal.vue/version_info.txt сайдкара); CHANGELOG 1.3.0 больше не «в разработке».

## Не закоммичено (в работе, 1.3)

### Экспорт строк пофайлово (CSV/JSON/PO)
Rust-команда `export_strings` — по файлу на исходник в подпапки `po/csv/json`, имена =
исходным. Одиночный экспорт (`exportCSV/JSON/PO` в actions.js) подставляет имя файла
(`defaultExportName`). Кнопка в `PairsWidget`.

### Редизайн карточки пары + реактивность
`PairsWidget.vue` переписан в карточку (заголовок, статус-бейдж черновик/собрано/изменено,
полоса прогресса, hover-действия). `loadPairs()` добавлен в saveFile/deleteManualString/
addManualString (фикс: статус не обновлялся после сохранения).

### Навигация + подсветка целевой строки
`scrollToBlock(id)` в store.js (финализация высот textarea + `behavior:'instant'` +
`flashBlock`). Класс `.row-flash`. Подключено: главный поиск (`jumpToFile`),
`focusBlockByIndex`, прыжки Header, AddStringModal.

### Реестр диагностик + автофиксы
`src/diagnostics.js` — единый RULES-реестр, мемоизированный `diagnose()`, `blockStatus`,
`applyFix`, `fixFile`, `hasBulkFixables`. Автофиксы: stripLeadingPrefix, restoreLeadingToken,
wrapToFit. `actions.js` делегирует. Editor рендерит единый блок диагностик с кнопками
«Исправить». Индикатор-состояние «Проверка» в Header (иконка !/?/✓ + выпадающее меню).

### Ручная отметка «перевод подтверждён»
DB-колонка `confirmed`. `DbEntry.confirmed`. `blockStatus`: confirmed && непустой →
translated. Кнопка-тоггл (галочка) в шапке блока. build пропускает идентичные пары.
Подтверждение пустой → авто-заполняет оригинал.

### Восстановление пропущенного файла — модалка «Файлы игры»
Rust `list_game_files` (обход game/, статусы extracted/empty/lang, детект языкового
суффикса). `FilesModal.vue` (поиск, фильтр-чипсы, клик → openEditor silent + showSourceModal).
SourceViewer «Добавить все» (addAllCandidates). `openEditor(opts.silent)`. Кнопка в дашборде.

### Предупреждение об осиротевших .rpy
`checkLooseRpy()` (actions.js) — детект loose .rpy без парного .rpyc → янтарный тост `warn`.
Тип тоста `warn` добавлен в GlobalMessages. Локаль `msg_loose_rpy`.

### Метка способа извлечения AST/Regex (последнее)
Сквозное поле `source`: экстрактор (main.py — ast/regex/engine=regex) → `ExtractedString`/
`DbEntry` → колонка `source` (+ ALTER-миграция) → чтение/запись во всех путях БД → тег в
шапке блока (`.src-ast`/`.src-regex`). Локали src_ast_hint/src_regex_hint ×6.
**Сайдкар пересобран.**

### Мелкие UI-правки (последняя сессия)
- Точки статуса в левой колонке редактора — ярче (насыщенные цвета + свечение).
- Единый ползунок громкости на вкладке аудио + синхронизация плееров (localStorage).
- Иконки `volume`/`volume-x` в Icon.vue.
- `list_game_files` фильтрует собственные скрипты RenForge (00_renforge_patch, renforge_*).
- Фикс читалки на старых играх: unrpyc `pickle_safe_loads` ASCII → latin-1. **Сайдкар пересобран.**

### Роадмап архитектуры + измеримость (0.0 / 0.1)
Роадмап универсальности извлечения/доставки зафиксирован в `_journal/09-roadmap.md` (фазы 0–3,
инварианты, ключевой инсайт «почему 100% недостижимо», таблица статусов) + ссылка из README.
Начаты пункты фазы 0 (измеримость + расчистка инфры):

**0.0 — bench.py починен.** `_testbench/bench.py`: путь `EXTRACTOR` → `renforge/src-tauri/tools/
extractor/main.py`; авто-дискавери игр из `GAMES_ROOT` (вместо устаревшего хардкода `GAMES`).
Проверено: list/run/snapshot/compare работают; 5 игр (butterflysoup 5965, ddlc 11000,
discipline 20818, refuge 4940, sayaka 1974); baseline пере-снят; compare = ALL MATCH. Движковые
версии авто-читаются из вывода экстрактора. Внутренняя инфра — в пользовательский CHANGELOG не идёт.

**0.1 — лог непокрытого + отчёт (opt-in диагностическая сборка).**
- Бэкенд (`lib.rs`): `RENFORGE_LOG_NOOP`/`RENFORGE_LOG_DIAG`; хелпер `_renforge_log_uncovered(chan,s)`
  вплетается в `python early` (реальный при diagnostic, иначе no-op); зовётся на промахах каналов
  K6 translate_string (`ui`, только если игра сама не перевела `_r==s`), K1 say, K3 input, K5 ui.text.
  Флаг `diagnostic` протянут через `generate_translations`(command) / `_core` / CLI (2×`false`) /
  preview (`false`). Команды `read_uncovered` (сверка лога с БД → `in_db`/`translated`) +
  `clear_uncovered`. Лог — `config.basedir/renforge_uncovered.log`. `cargo check --workspace` чисто.
- Фронт: `diagnosticBuild` (стор, persist localStorage) + `showUncoveredModal`; `actions.js`
  buildMod/generateTranslations шлют `diagnostic`. `UncoveredModal.vue` (самодостаточные scoped
  `uc-*` стили): список непокрытого, фильтр «кандидаты» (`in_db=false`) / всё, поиск, обновить,
  очистить лог, добавить строку в ручные (`addManualString`). Dashboard: тумблер диагностики
  (иконка eye) + кнопка отчёта (иконка search) в блоке сборки. App.vue рендерит модалку.
- Локали ×6: 9 ключей (diag_build, uncovered_title/_hint, uncov_filter_candidates/_refresh/_clear/
  _empty/_in_db/_added) → **424×6** (паритет проверён скриптом-дифом). `node --check` чисто.
- **Требует прогона в игре:** собрать мод с диагностикой ON → пройти игру → открыть отчёт
  «Непокрытый текст». До прогона лог пуст (это норма). Детали: 01 (бэкенд), 03/04 (фронт),
  06 (решения), 09 (роадмап-статус).

## Разбор бага Katawa Shoujo (диагностика, не код RenForge)
Игра падала при запуске (`init offset = -2`, `expected ':' not found`, Ren'Py 6.10.2e).
Причина: 264 декомпилированных `.rpy` в game/, созданных ВНЕШНИМ прогоном unrpyc (не
RenForge — мы пишем декомпиляцию только в `.renforge/decomp/`). `init offset` — артефакт
unrpyc, старый движок его не знает. Решение: снесли все .rpy + перевод + БД для чистого
теста извлечения. Оставлен `data.rpa.renforge-disabled`.
Ср. `08-edge-cases.md` → «qa.py засорял папки игр декомпиляцией» — тот же класс инцидента
(там было удалено 623 мусорных .rpy), подтверждает первопричину.

## Фидбэк тестера 1.2 (в работе)
- **[СДЕЛАНО, не закоммичено] Достижения:** убран форс `config.developer`/`config.console`
  из патча (`apply_renforge_patch_core`) — ломал Steam-достижения. Детали в 08-edge-cases.
  Ждёт визуальной проверки тестером.
- **[СДЕЛАНО, не закоммичено] Дубль-английский (multi-key delivery):** реализовано —
  экстрактор собирает `alt_texts` из одноязычных источников (base + tl/<same-lang>) по
  translation id (сэмпл-детект одноязычности, порог 0.5), доставка регистрирует перевод под
  всеми вариантами, в редакторе показ alt-контекста. Сквозь все слои (экстрактор → БД →
  доставка → фронт), 6 локалей. Сайдкар пересобран. Проверено (негатив ButterflySoup,
  синтетика, юнит-мок). Детали в 08-edge-cases. Ждёт проверки тестером.

## Аудит кодовой базы (полный проход по исходникам)

Сквозная сверка ВСЕХ первичных исходников с журналом: Rust-бэкенд (lib.rs ~2.45k строк, db/tm/
models/error/main), Python-экстрактор (main.py, unpickler.py), фронт (store/actions/diagnostics/
App/main), locales.js (все 6 языков), все 18 `.vue`-компонентов, style.css. Журнал в целом
**точен**; внесены точечные правки и подтверждён/опровергнут ряд подозрений.

**Исправлено в журнале (дрейф от кода):**
- `05`+`01`: колонка `alt_texts` пропущена в списке ALTER-миграций; `upsert` — не 13, а **14** колонок.
- `01` (tm.rs): таблица `tm` несёт ещё `source_lang`/`updated_at` + индекс `idx_tm_target`;
  `tm_fill` метит `status='outdated'`+`prev_original=original`.
- `02`+`07`: **дубли `explore_ast`/`extract_layeredimage_strings` ОПРОВЕРГНУТЫ** — по одному определению.
- `04`+`06`: жалоба «AI-промт следует только первой пачке» **по коду не воспроизводится** —
  `runChunked` шлёт полный system каждую пачку (уточнить у тестера, о чём была жалоба).
- `03`: локали — **410 ключей × 6 языков, идеальный паритет** (0 пропусков/лишних/дублей, проверено скриптом-дифом).

**Добавлено (было недокументировано):**
- `01`: инлайн юнит-тесты в lib.rs (`cargo test`); `ingest` через `ON CONFLICT(id) DO UPDATE`
  сохраняет перевод при переизвлечении (id не изменился); `font_scripts` = **26** письменностей.
- `02`: `detect_engine_version`, `GAME_META`/`capture_game_meta` (имя/версия игры с эвристикой),
  извлечение legacy `displayDict` (katawa-style) — при том что доставка на такие движки не реализована.

**Реальные кандидаты на чистку** (детали — в разделе ниже): ~~мёртвый Vue Flow (CSS+deps)~~ —
**удалено** (коммит `44d304a`), ~~`extract_targeted_use_args` (не вызывается)~~ — **удалено**,
~~лишний импорт `clean_filename`~~ — **удалено**, ~~устаревший doc-`FontInfo`~~ — **актуализирован**
тем же коммитом. Весь список чистки из аудита закрыт.

### Roadmap 1.3 — репорт пропущенных файлов экстрактора (Option B, реализовано в исходниках)
Экстрактор (`main.py::process_directory`) теперь кладёт `skipped_files` (пути относительно
`game/`, нормализованные слэши) в выходной JSON — и на сбой `explore_ast`, и на пустой
`load_ast` (раньше молча терялся, не считался вообще). `ExtractedData.skipped_files: Vec<String>`
(models.rs, `#[serde(default)]` — обратная совместимость со старым сайдкаром, который поле не
пишет). `extract_and_ingest_project` (lib.rs) сменил тип возврата с голого `i64` на
`ExtractResult { total, skipped_files }` — читает оба поля из ОДНОГО разбора JSON (не два раза).
Фронт (`actions.js::prepareProject`) читает `result.total` вместо `total`, и при непустом
`skipped_files` кроме обычного успех-тоста показывает `warn`-тост со списком (первые 5 + «…»).
Локаль `msg_extractor_skipped` ×6. **Проверено:** `cargo check` (полный workspace, включая GUI-
таргет — на Linux потребовались системные GTK/OpenSSL dev-пакеты и временные заглушки под sidecar-
бинари для линуксового триплета, удалены после проверки, не коммитятся); `vite build` фронта —
чисто. **Требует пересборки сайдкара** (`rpyc_extractor.exe`, PyInstaller/Windows) владельцем
перед тестом/коммитом — правка тронула `main.py`.

**Перед релизом 1.3 — бамп версии в 5 местах** (сейчас везде `1.2` / `1.2.0`):
`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`,
`src/components/Header.vue` (`<sup class="version">1.2</sup>`),
`src/components/AboutModal.vue` (`const APP_VERSION = '1.2.0'`; используется и в FALLBACK-тексте атрибуции).

## Изъяны из аудита (очередь на фикс — делаем по порядку)

Найдено во втором проходе аудита («где ломается / ведёт себя неверно»). Порядок = приоритет.

1. **[СДЕЛАНО] Нет изоляции ошибок в главном цикле извлечения.** `main.py`
   `process_directory`, ветка `else` (~533): `for filepath in rpyc_files: … explore_ast(tree,
   results=extracted_data["strings"])` НЕ обёрнут в try/except — в отличие от `attach_alt_texts`
   (~547) и regex-ветки. Исключение на ОДНОМ файле (нестандартный узел → AttributeError/
   TypeError, RecursionError) пробрасывается и валит весь `process_directory`; строки пишутся в
   JSON только ПОСЛЕ цикла → теряется ВСЁ извлечение, юзер видит `extractor_error` и 0 строк.
   Фикс: per-file try/except + continue + буфер (extend на успехе) + лог пропущенных в stderr.
   Для валидных файлов вывод обязан остаться байт-в-байт прежним (проверка — before/after-дифф).
   **Сделано и проверено (12.07.2026):** в `main.py` per-file `try/except` + буфер `tmp`
   (extend в общий список только при успехе) + лог пропущенных в stderr. Регрессия:
   before/after-дифф на 5 реальных играх (ButterflySoup / DDLC / Discipline / Refuge of Embers /
   Sayaka — 44 697 строк) → вывод байт-в-байт идентичен. Позитивный тест: симуляция сбоя
   `explore_ast` на одном файле → остальные извлеклись, краха нет, пропуск залогирован. Сайдкар
   пересобран (PyInstaller через venv) и развёрнут в `bin/`; smoke frozen-exe = 1974 строки
   (эталон Sayaka). Строку в CHANGELOG добавляет владелец.
2. **[СДЕЛАНО] Перенесённые (fuzzy) и TM-строки не доставлялись до ревью.** `build_runtime_rpy` шлёт только
   `status='translated'`; migrate-fuzzy и `tm_fill` ставят `status='outdated'` (+`prev_original`).
   Пока стоит `prev_original` — строка в игру НЕ попадает (снимается правкой `onInput` или кнопкой
   «Отметить проверенным» `resolveOutdated`). Тихо (отчёт сборки не говорит, сколько удержано) +
   «нельзя проверить в игре, т.к. не доставлено». Не краш — нужно продуктовое решение (минимум:
   «доставлено X / удержано на проверку Y» в отчёте; опционально — доставлять и outdated).
   **Решение (вариант B, сделано):** `build_runtime_rpy` доставляет и `outdated`
   (`status IN ('translated','outdated')`), считает `review` и возвращает
   `BuildCounts {say, ui, review, skipped_bad}`; `buildMod` показывает локализованную сводку
   (доставлено / требуют проверки: N / пропущено небезопасных). Локали ×6
   (build_delivered / build_review / build_skipped_bad). Пометка «к проверке» в редакторе
   сохранена. Проверено: `cargo check` (lib+cli), локали 413×6, `node --check`. CHANGELOG ru+en.
   Рационал — в 06 «Доставка».
3. **[СДЕЛАНО (пометка в редакторе)] Дубли оригинала с разными переводами — доставится один.** `say_map`/`strings_map` ключуются
   по тексту (`or_insert`, первый побеждает), `SELECT` без `ORDER BY`. Врождённое ограничение
   текстовой подстановки; какой вариант победит — по порядку выдачи SQLite.
   **Сделано (пометка):** бэкенд `get_duplicate_originals` (GROUP BY original → {count, variants});
   `dupMap` в сторе (фоново на открытии файла и после сохранения). В редакторе строки-дубли
   получают бейдж (иконка + N, «перевод общий»), а при разных переводах (variants>1) —
   бейдж-конфликт «!» + предупреждающая строка `dup_conflict` (локали ×6). Само ограничение
   доставки не меняли — теперь оно видимо переводчику. Проверено: cargo check, локали 415×6, node --check.
4. **[СДЕЛАНО] `escape_py_double` не экранировал U+2028/U+2029.** Весь перевод — один файл
   `renforge_translations.rpy` (архитектурно «всё или ничего»: сломанный литерал роняет ВСЕ
   переводы). **Уточнено эмпирически:** сам CPython U+2028/U+2029 в `u"..."` терпит (compile
   не падает), НО это разделители строк для `str.splitlines()` → построчный лексер (Ren'Py
   читает .rpy построчно) мог «разорвать» логическую строку. **Фикс:** `escape_py_double`
   переводит их в `\uXXXX` (тот же рантайм-символ, значение не меняется). Юнит-тест
   `test_escape_py_double` расширен; `cargo test` — 4/4 ok. Радиус «всё или ничего» остаётся
   архитектурным, но главный триггер убран.
5. **Переизвлечение: `_N`-суффикс зависит от порядка** для точных дублей под одной меткой / старых
   движков без `node.identifier`. Узко (контентные id спасают в общем случае). Отложено.

**Инфра-замечание (нашлось при фиксе #1):** `_testbench/bench.py` устарел — `EXTRACTOR` указывает
на `extractor/scr/main.py` (НЕ существует; исходник давно в `renforge/src-tauri/tools/extractor/`),
а `GAMES` не совпадает с реально лежащими в `Игры для тестов` (ButterflySoup, DDLC, Discipline,
Katawa, Refuge of Embers, Sayaka). Регрессию извлечения гоняем прямым before/after-диффом, пока
bench не поправлен (кандидат на отдельную мелкую задачу). **[СДЕЛАНО — roadmap 0.0]** bench.py
починен (пути + авто-дискавери игр из GAMES_ROOT); см. раздел «Роадмап архитектуры (0.0 / 0.1)»
выше и `09-roadmap.md`.

## Кандидаты на чистку (из аудита при составлении журнала)
- **~~Дубли в `unpickler.py`~~ — ОПРОВЕРГНУТО (аудит):** `explore_ast` и
  `extract_layeredimage_strings` определены по ОДНОМУ разу (строки 594 и 564). Дублей нет —
  прежнее подозрение было ошибкой наблюдения (рекурсивные вызовы приняли за определения).
- **Мёртвый код в экстракторе — УДАЛЁН:** `extract_targeted_use_args` (ФИКС №3, unpickler.py) не
  вызывалась нигде — вытеснена инлайновой обработкой `SLUse` в `explore_ast`; `clean_filename` —
  лишний импорт в main.py (используется только внутри unpickler.py) — тоже удалён.
- **~~Мёртвый Vue Flow~~ — УДАЛЕНО (подтверждено аудитом — координаты):** был CSS-блок «VUE FLOW OVERRIDES»
  в `src/assets/style.css` (строки **565–598**): `.renforge-flow`, `.vue-flow__*`
  (node/minimap*/controls*), `.custom-node`, `.node-faded/-title/-content/-file/-empty`,
  `.interactive-node`, `.file-tag/-warning/-error/-success`, `.tag-tl`, `.title-sync/-warn`,
  `.warning-icon/.success-icon` + 4 зависимости package.json
  (`@vue-flow/background|controls|core|minimap`). Grep-проверено: НИ ОДИН класс блока и ни один
  `@vue-flow` не используется в `.vue`/`.js`. Граф файлов заменён дашбордом в 1.2. Удалено целиком
  (коммит `44d304a`).
- **~~Устаревший doc-комментарий `FontInfo`~~ — актуализирован** тем же коммитом (`44d304a`):
  перечисляет 9 письменностей, а `font_scripts` (lib.rs) фактически пробует 26.

## Идеи/фичи в очереди
См. «Отложенные пункты» в `06-decisions-log.md`.
