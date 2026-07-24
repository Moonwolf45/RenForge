# Фронт-ядро: store.js / actions.js / diagnostics.js

Обзор: `main.js` тривиален — `createApp(App).mount('#app')`, без роутера/плагинов.
«Роутинг вкладок» в `App.vue` — не vue-router, а переключатель по `currentMode` через
`v-if`: `Dashboard` / `Editor` / `ImageGallery('gallery')` / `AudioGallery('audio')`.
Поверх — модалки, каждая по своему булеву-флагу (`isAiModalOpen`, `showUpdateModal`,
`showTmModal`, `showSourceModal`, `showAddStringModal`, `showDeliveryHooksModal`,
`showAboutModal`, `showFilesModal`, `showUncoveredModal`), плюс фон-оверлей закрытия поповеров по `activePopover`.
`accentStyle` (computed) прокидывает акцент в CSS-переменные `--accent/--accent-hover/
--accent-contrast`. `handleContextMenu` глушит нативное контекстное меню всюду, кроме полей
ввода/выделения. `locales.js`: объект `locales` с 6 языками (`ru, en, zh, ja, es, pt`),
каждый — плоская карта `ключ: строка`; `t(key)` реактивно читает `uiLang.value`, падает на
`en`, затем на сам ключ. `SCRIPT_CODES` — порядок кодов письменностей (**26 кодов**), синхронный с Rust `font_scripts`
и с 26 локале-ключами `script_*` (поддержка всех 26 письменностей — сквозная).
**Проверено (аудит, скрипт-диф по union ключей):** все 6 языков синхронны ключ-в-ключ —
по **424 ключа**, 0 пропущенных, 0 лишних, 0 дублей. Правило владельца «6 локалей синхронно»
соблюдается.

## store.js — реактивное состояние и утилиты
Модуль-синглтон на `ref`; состояние вне компонентов. Часть настроек зеркалится в
`localStorage` (глобально и/или пер-проектно через `getProjectKey`).

Ключевые состояния:
- **UI:** `uiLang`, `uiTheme`, `uiAccent`, `targetScript='auto'`, `diagnosticBuild` (opt-in
  диагностическая сборка, persist localStorage `renforge_diagnostic_build` — roadmap 0.1).
- **Языки:** `sourceLang`, `targetLang` (пер-проектные), `availableLanguages`, `targetLangCollision`
  (computed — `target ∈ available_languages` без регистра, исключая 'original'; для предупреждения о
  мультиязычной коллизии, roadmap 1.2).
- **Навигация/модалки:** `currentMode='dashboard'`, `activePopover`, `showFontPanel`, все
  `show*Modal`, `manualEditTarget` (null → режим добавления ручной строки).
- **Проект:** `projectPath`, `isProcessing`, `isExporting`, `projectFiles`
  (rpa/rpyc/rpy/tl), `fileStats`, `charMap`.
- **Пары:** `translationPairs`, `activePair`, `MANUAL_FILE='__renforge_manual__'`.
- **Редактор:** `parsedBlocks`, `currentFilePath`, `rawFileText`, `isEditorLoading`,
  `hideTranslated`, `focusedBlockId`, `flashBlockId`, `editorDirty`, `lastSavedAt`,
  `editorResizeTick`, `searchQuery`, `searchResults`.
- **Пер-проектные списки** (авто-`watch` в localStorage): `hiddenFiles`, `completedFiles`,
  `fileNotes`, `glossary`, `hiddenImages`, `hiddenAudio`, `hiddenFolders`, `showHidden`,
  `showHiddenMedia`.

Функции:
- **`showMsg(type, text, timeout=8000)`** — стек тостов. Пустой text снимает текущий
  липкий тост (отмена экспорта). `timeout===0` = липкий тост операции: обновляется на
  месте через единственный `stickyId` (не копится). Таймерный тост = финал: снимает
  липкий, добавляет, автоскрытие. Типы: `success`/`error`/`warn` (warn — только в
  checkLooseRpy). Связи: removeToast/closeMsg/toastTimers.
- **`flashBlock(id)`** — «вспышка» блока: сброс в null + rAF-переустановка, чтобы CSS-
  анимация перезапускалась при повторной навигации к той же строке; гаснет ~1600мс.
- **`scrollToBlock(id)`** — центрирование + flashBlock. ГРАБЛИ: перед скроллом
  принудительно доводит высоту ВСЕХ `.transparent-input` до финальной (height:auto→
  scrollHeight), иначе ленивый авторост (IntersectionObserver) растягивает соседние
  textarea во время прокрутки и цель уезжает (строка 217 промахивалась в ~330). Двойной
  rAF: сперва высоты, затем центрирование. **`behavior:'instant'`** намеренно против CSS
  `scroll-behavior:smooth` — мгновенный точный прыжок.
- **`getProjectKey`/`loadProjectSettings`/`safeParseJSON`** — пер-проектное хранилище;
  для свежего проекта sourceLang/targetLang остаются '' (осознанный выбор до извлечения).
- **`darkenHex`/`resolveAccent`/`contrastFor`** — акцент: uiAccent хранит либо ключ
  пресета, либо `#rrggbb`; resolveAccent понимает оба; contrastFor по яркости sRGB (WCAG)
  выбирает чёрный/белый текст на заливке.
- **`getFileName`/`getRelativePath`/`getFolderFromPath`** — путевые утилиты (нормализуют
  `\`→`/`).
- **`ACCENTS`** — приглушённая пастель (мягче на AMOLED, яркое «выжигает»). **`FUNNY_PROMPTS`**
  — пасхалка-персоны (сохранены плейсхолдеры/нумерация, чтобы разбор ответа не ломался).

## actions.js — оркестрация (Tauri invoke + диалоги)
Паттерн: try/catch → showMsg('error'), флаг isProcessing. **КЛЮЧ реактивности статусов:**
`loadPairs()` дёргается после каждой операции, меняющей БД, иначе карточки пар
(прогресс/«изменено») устаревают.

- **`refreshProject()`** — scan_project + get_character_mapping + get_translation_stats +
  discover_source_languages (доступно ДО извлечения для loose-file игр) + loadPairs.
- **`loadPairs()`** — list_translation_pairs → translationPairs/activePair. Вызывается из
  switchPair/deletePair/prepareProject/saveFile/addManualString/deleteManualString/tmFill/removeMod.
- **`switchPair`/`deletePair`** — legacy→use_legacy_db; обычная→set_active_pair (+source/
  target). deletePair: ask, legacy нельзя.
- **`prepareProject()`** — extract_and_ingest_project теперь возвращает `{total, skipped_files}`
  (roadmap 1.3, было — голый total): успех-тост по `total`, и при непустом `skipped_files` —
  доп. `warn`-тост со списком (первые 5 + «…», локаль `msg_extractor_skipped`). Конвейер:
  (1) гейт обоих языков (страховка от контаминации);
  (2) prepare_writable + is_path_writable ДО записи (read-only/UAC иначе молча теряет
  файлы); (3) распаковка .rpa с липким прогрессом, затем повторный discover_source_languages;
  (4) защита source===target (ask); (5) extract_and_ingest_project. Локализует коды ошибок
  game_dir_missing/extractor_spawn_failed/extractor_error. В конце refreshProject + checkLooseRpy.
- **`checkLooseRpy()`** — осиротевшие .rpy без парного .rpyc (сопоставление rpyStem) →
  showMsg('warn'): совет запустить игру раз и переизвлечь для точного AST.
- **`generateTranslations`/`buildMod(fontRemaps=[])`** — generate_translations
  (+apply_renforge_patch у buildMod); передаёт `diagnostic: diagnosticBuild.value` (лог непокрытого,
  roadmap 0.1). fontRemaps: {source=rel_path шрифта, target=путь|null=встроенный DejaVuSans}.
- **`exportTranslation(pair, mode)`** — 'full'/'mod'; предупреждение при is_built&&is_dirty;
  санитизация имени; подписка на событие export_progress; коды exists/cancelled/done/nospace;
  isExporting → кнопка «Отмена».
- **`cancelExport()`** — cancel_export (остановка в ближайшей итерации).
- **`tmFill()`** — tm_fill (точные совпадения TM → непереведённые, «к проверке»); при n>0
  refreshProject.
- **`removeMod(pair)`** — ask + remove_renforge_mod (убирает доставку; БД и tl/<target>
  остаются) + loadPairs.
- **`migrateTranslations(oldPath)`** — migrate_translations (fuzzy-перенос) + refreshProject.
- **Ре-экспорт диагностик:** getOriginalTags/getMissingTags/getExtraInterps из diagnostics.js;
  getBlockStatus = обёртка над blockStatus.
- **`openEditor(dbFilePath, opts={})`** — currentMode='editor', get_translations_for_file →
  parsedBlocks. НЮАНС: `opts.silent` подавляет тост «нет строк» (намеренное открытие
  не-извлечённого исходника — пустота норма); для MANUAL_FILE тоже не показывается. При
  ошибке откат в dashboard.
- **`addManualString(...)`** — ручная строка, text-keyed доставка (file_path/line_number
  косметические, для навигации); id='manual_'+djb2. Цель = текущий файл → вставка в
  parsedBlocks по line_number + editorDirty; иначе → upsert в БД + fileStats + loadPairs.
- **`isManualString`/`updateManualString`** — предикат id.startsWith('manual_'); правка на
  месте (id не меняется — матч по тексту).
- **`deleteManualString`** — ask + delete_translations + splice + fileStats + loadPairs.
- **`saveFile()`** — БЛОКИРУЕТ сохранение при статусе error (msg_cannot_save_errors);
  проставляет status, upsert_translations_batch, локально пересчитывает прогресс-бар,
  editorDirty=false, lastSavedAt, loadPairs, фоновый tm_contribute (ошибки глотаются).
- **`exportAllStrings()`** — export_strings активной пары (по файлу × CSV/JSON/PO); пара
  переключается вызывающим (PairsWidget).
- **`defaultExportName(ext)`** — имя открытого файла + расширение (script.rpy.po).
- **`exportCSV`/`importCSV`** — ID;Original;Translation, `\n`→`[BR]`, анти-CSV-инъекция
  (префикс `'` для `=+-@`).
- **`exportJSON`/`importJSON`** — {id, original, translation}; матч по id.
- **`exportPO`/`importPO`** — gettext: `#: file:line`, `msgctxt`=id (уникальность при равных
  оригиналах), `#, fuzzy` для outdated; poEscape/poUnescape/parsePO (многострочные
  продолжения); импорт по id===ctxt, пустой msgstr не трогает.
- **`jumpToFile`** — фактически в Dashboard.vue (не в actions.js): openEditor +
  setTimeout(scrollToBlock, 500).

## diagnostics.js — реестр диагностик + автофиксы
- **Токены:** `extractTags` (`[...]`&`{...}`), `extractInterps` (`[...]`), `getMissingTags`
  (теги оригинала, отсутствующие в переводе), `getExtraInterps` (лишние `[var]` → Ren'Py
  KeyError; `{...}` не проверяем).
- **RULES:** `missing-tag` (error, fix=restoreLeadingToken, bulk), `extra-interp` (error,
  fix=stripLeadingPrefix, bulk), `ui-overflow` (warning, fix=wrapToFit, НЕ bulk). error
  блокирует сохранение через blockStatus; фиксы контекстные (только ВЕДУЩИЙ префикс/токен),
  иначе null и кнопка «Исправить» скрыта.
- **`diagnose(block)`** — мемоизация в `_cache` по ключу `id\0original\0translation`;
  предохранитель: при size>4000 полный сброс (память важнее). `fixable` = фикс реально
  вернул непустой результат (не «есть fix»); `bulkFix`=fixable&&bulk. `clearDiagnostics()`
  сбрасывает кэш.
- **`blockStatus(block)`** — error > любая error-диагностика; confirmed&&hasTr → translated
  (бьёт «перевод==оригинал», но НЕ ошибки); пусто/перевод==оригинал → untranslated;
  prev_original → outdated; иначе translated. Учитывает confirmed и (косвенно) source
  через prev_original.
- **`applyFix`/`fixFile`/`hasBulkFixables`** — одиночный фикс; массовая починка bulk-правил
  (мутирует translation, возвращает число); наличие bulk-фиксов для кнопки «Исправить файл».
- **`stripLeadingPrefix`** — срезает прилипшие ведущие префиксы-эхо (`[ENGINE]:`/`[name]`)
  подряд, останавливается на совпадении с ведущим бракетом оригинала (не убить легитимный
  `[player]`). Для extra-interp.
- **`restoreLeadingToken`** — восстанавливает потерянные ВЕДУЩИЕ токены оригинала, только
  если ВСЕ отсутствуют (иначе null, не дублировать); пустой перевод не трогает. Для missing-tag.
- **`wrapToFit`** — автоперенос длинного UI-перевода под ширину оригинала; `visibleWidth`
  меряет через canvas (20px sans-serif), игнорируя `{...}`; фоллбэк по длине символов если
  canvas null. `uiOverflowWarn` триггерит только для block_type==='ui', непустого перевода≠
  оригиналу, без `\n`, при >orig*1.3 и +≥4 символов.
