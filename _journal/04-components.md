# Компоненты (src/components)

Vue 3 `<script setup>` + Tauri; бэкенд через `invoke(...)`, общее состояние в `store.js`,
действия в `actions.js`, диагностики в `diagnostics.js`, i18n `t()`. Настройки — в
`localStorage` с префиксом `renforge_`.

## `Dashboard.vue`
- **Роль:** главный экран проекта — карточка игры, пайплайн (извлечь → перевести → собрать
  мод), статистика, список файлов, панель шрифтов, память переводов.
- **Состояние:** локальные `projectFonts`, `fileFilter`/`fileSort`/`sortDir`, `imgListRaw`/
  `audListRaw`, `tmCount`, `gameName`/`gameVersion`/`engineVersion`; из стора `fileStats`,
  `hidden*`, `targetLang`/`sourceLang`/`targetScript`. Computed `overall`, `counts`,
  `visibleFiles`, `imgStat`/`audStat`, `hiddenCounts`.
- **Логика:** `doExtract`→prepareProject, `doBuildMod`→buildMod(fontRemaps), `doTmFill`→
  tmFill, поиск через search_in_db, openEditor. invoke: get_project_meta, get_images_list,
  get_audio_list, get_project_fonts, tm_count, search_in_db. **Блок сборки** несёт тумблер
  диагностики покрытия (иконка eye, тоггл `diagnosticBuild`) и кнопку отчёта «Непокрытый текст»
  (иконка search → `showUncoveredModal`), рядом с хуками / шрифтами / сборкой (roadmap 0.1).
- **Нюансы:** скрытые файлы/папки в общий зачёт прогресса и `counts` НЕ входят (тумблер
  влияет лишь на список). `langToScript()` — эвристика язык→письменность; `DEJAVU_SCRIPTS`
  определяет, что покроет встроенный DejaVu → дефолтный режим шрифта (keep/default/custom).
  Режим `custom` открывает диалог выбора файла. `stepState()` — визуальные состояния шагов
  (locked/active/done). После jumpToFile центрирование по строке отложено setTimeout(500) —
  ждёт отрисовки блоков. **Баннер мультиязычной коллизии** (`.lang-collision-bar`, roadmap 1.2):
  при `targetLangCollision` под инфо-баром игры показывается янтарное предупреждение (`collisionMsg`
  = `lang_collision_warn` с подстановкой имени языка) — целевой язык уже встроен в игру.

## `Editor.vue`
- **Роль:** основной экран перевода одного файла: сайдбар-навигация по строкам, панель
  блоков (оригинал/перевод), сайдбар глоссария.
- **Состояние:** из стора parsedBlocks, hideTranslated, focusedBlockId, charMap, glossary,
  newTerm, editorDirty, editorResizeTick, flashBlockId, currentFilePath/MANUAL_FILE, dupMap.
  Локально glossaryOpen, editorSearch. Computed totalCount/doneCount/pct, matchCount.
- **Логика:** статус блока — getBlockStatus; диагностики — diagnose/applyFix/clearDiagnostics.
  `blockVisible` фильтрует по «скрыть переведённые» + поиску (id/оригинал/перевод/who).
  Клавиатура: Enter → следующий блок (Ctrl+Enter — только непереведённые), Esc → blur.
  `setChannel` переопределяет канал доставки (auto/say/ui/both). Глоссарий: подсветка
  терминов, клик по подсвеченному слову вставляет перевод в позицию курсора.
- **Нюансы:** `src-tag` AST/Regex показывает источник извлечения блока (разные title-
  подсказки). **Бейдж дубликата** (`dupInfo`/`dupTitle` из `dupMap`, грузится на открытии/сохранении):
  если оригинал встречается в проекте >1 раза — счётчик-бейдж (перевод общий); при variants>1 —
  бейдж-конфликт «!» + строка-предупреждение `dup_conflict` (в игру уйдёт один вариант, изъян #3).
  `toggleConfirmed` — тоггл «перевод подтверждён»; подтверждение пустой строки
  вписывает оригинал (translation===original → доставка no-op). Кнопки «Исправить»
  применяют автофикс одной диагностики. Три оптимизации на больших файлах: (1) `v-autogrow`
  — ленивый авто-рост textarea через IntersectionObserver (была главная причина долгого
  открытия); (2) мемоизация подсветки глоссария `hlCache` по id блока (сброс при смене
  глоссария/файла); (3) перед навигацией к блоку принудительно доводит высоту всех видимых
  textarea, иначе центрирование промахивалось. `editorResizeTick` — сигнал массово
  пересчитать высоты после пакетного перевода/импорта. `outdated`-блоки показывают прежний
  оригинал (diff) либо метку «из памяти переводов».

## `ImageGallery.vue`
- **Роль:** галерея изображений с папками, поиском, пагинацией, drag&drop локализованных
  картинок и лайтбоксом с зумом/панорамой.
- **Состояние:** galleryImages, gallerySearch, gallerySelectedFolder, galleryCurrentPage
  (100/стр); лайтбокс — lightboxImg, lightboxShowOriginal, zoom/minZoom/maxZoom, naturalW/H,
  isDragging. Из стора hiddenImages/hiddenFolders/showHiddenMedia.
- **Логика:** invoke get_images_list, import_localized_image, delete_localized_image,
  open_in_explorer. getImgSrc/lightboxSrc через convertFileSrc. Нативный OS drag&drop через
  getCurrentWebview().onDragDropEvent — по позиции курсора находит карточку (cardPathAtPoint,
  деление на devicePixelRatio). Клавиши ←/→/Esc в лайтбоксе.
- **Нюансы:** лайтбокс самодельный: зум колёсиком вокруг курсора (zoomAround), панорама
  перетаскиванием, «вписать» (fitToViewport), тумблер оригинал/локализованное. Подложка
  прозрачности переключается (`lightboxBg`, localStorage): мягкая тематическая шахматка
  (`color-mix(var(--text-main) 6%)` — адаптивна к теме) / тёмный / светлый — для оценки
  прозрачных PNG. Классы `.bg-checker/.bg-dark/.bg-light` + свотч-переключатель `.lb-bg-btn`. При листании
  картинка предзагружается через new Image(), navToken защищает от гонки (src меняется
  только после загрузки — не мерцает). Проверка расширения дропа (IMG_EXTS). Стиль
  глобальный (не scoped) — шахматка для прозрачных PNG.

## `AudioGallery.vue`
- **Роль:** галерея аудио с папками, поиском, пагинацией, сопоставленным текстом реплики,
  drag&drop и общим регулятором громкости.
- **Состояние:** audioFiles, audioSearch, audioSelectedFolder, audioCurrentPage; `audioVolume`
  (единая громкость, из localStorage), lastNonZeroVolume. Из стора hiddenAudio/hiddenFolders/
  showHiddenMedia.
- **Логика:** invoke get_audio_list, import_localized_audio, delete_localized_audio,
  open_in_explorer. OS drag&drop как в галерее.
- **Нюансы:** единый ползунок громкости синхронизируется в обе стороны — `applyVolumeToAll()`
  раскатывает по всем `<audio>` со сравнением по допуску (не зациклить событие), штатный
  регулятор плеера через `@volumechange`→`onAudioVolumeChange` поднимает значение обратно в
  общий ползунок (тоже допуск против обратной волны). `toggleMute` помнит последнюю ненулевую
  громкость. Одновременно играет один плеер: `onAudioPlay` паузит остальные. Новые плееры
  (пагинация/фильтр) получают громкость через `watch(paginatedAudio)`. Карточка показывает
  mapped_text/mapped_script либо «не сопоставлено».

## `SourceViewer.vue`
- **Роль:** модалка-читалка исходника: вкладка «оригинал» (декомпилированный .rpyc с
  подсветкой) и «renforge» (предпросмотр сгенерированного перевода), с минимапом.
- **Состояние:** tab, original/renforge, loadingMap/errorMap по вкладкам, showExtracted/
  showCandidates, navIdx/candNavIdx, vp (рамка минимапа). Из стора currentFilePath,
  parsedBlocks, projectPath, targetLang.
- **Логика:** invoke decompile_rpyc, preview_generated_translations. Собственный лёгкий
  токенайзер Ren'Py/Python (tokenize, splitString, KEYWORDS) даёт подсветку без внешних либ;
  renderedLines считает уровни отступов. Извлечённые строки подсвечиваются по line_number из
  parsedBlocks, с навигацией «пред/след».
- **Нюансы:** «кандидаты — возможно пропущено» (эвристика): detectCandidate + looksTexty
  флагуют высокосигнальные `_()`, say-строки и screen-текст (text/textbutton/tooltip/label),
  отсекая пути/ассеты/идентификаторы/нетекст (NONTEXT_KW) — помощник, не второй экстрактор.
  addCandidate/addAllCandidates→addManualString; массовое добавление итерируется по снимку
  (candidateMap пересчитывается по ходу). Минимап рисуется на canvas вручную (силуэт кода,
  маркер извлечённых слева зелёным, кандидатов справа амбером), клик/перетаскивание скроллит.
  Ошибки маппятся: rpyc_missing→source_no_rpyc, «В базе нет»→source_no_preview.

## `Header.vue`
- **Роль:** верхняя панель; содержимое зависит от currentMode (dashboard/gallery/audio vs
  editor) — лого, переключатель разделов, настройки, помощь, экспорт/импорт, сохранение,
  индикатор проверки (QA).
- **Состояние:** из стора currentMode, activePopover, uiTheme/uiLang/uiAccent/targetLang/
  sourceLang/targetScript, parsedBlocks, editorDirty, lastSavedAt, isAiModalOpen. Локально
  targetLangSelect (пресет vs custom), пасхалка eggDrops.
- **Логика:** openProjectFolder→dialog + loadProjectSettings + refreshProject; closeEditor
  со спросом при несохранённых; экспорт/импорт CSV/JSON/PO; saveFile; настройки в localStorage.
- **Нюансы:** индикатор «Проверка» — `qaState`/`qaIcon`: ошибки → `!` (alert), предупреждения
  (outdated + warning-диагностики) → `?` (help), иначе → `✓` (check); в поповере — прыжки к
  следующей ошибке/проверке/предупреждению и массовая починка файла (fixFileAll→fixFile,
  только безопасные bulk-автофиксы). `targetLangSelect` синхронизируется watch'ем с targetLang.
  Пасхалка: 30 кликов по лого → кувырок окна, «ливень» логотипов (Teleport), тихая подмена
  системного промпта нейросети на случайный из FUNNY_PROMPTS (чинится «Сбросить к стандарту»
  в AI-модалке) + ехидный тост.

## `AiModal.vue`
- **Роль:** AI-ассистент перевода: три режима — локальный Ollama, облачный OpenAI-совместимый
  API, ручной (копи-паст).
- **Состояние:** aiTab, диапазон aiStart/aiEnd + onlyUntranslated, chunkSize, ollama* (URL/
  model/temp/system), api* (URL/key/model/temp), includeSpeaker, флаги isOllamaTranslating/
  isApiTranslating/aiCancel, aiInput. Из стора parsedBlocks, targetLang, glossary, charMap.
- **Логика:** runLocalLLM → fetch к `${ollamaUrl}/api/generate`; runApiLLM → invoke
  llm_chat_request. Оба через общий runChunked(sendFn, busyRef). buildSystem подставляет
  {target_lang}/{count}/{glossary} в промпт (DEFAULT_SYSTEM). Настройки в localStorage.
- **Нюансы:** перевод пачками (chunkSize, дефолт 25): большие диапазоны бьются на чанки,
  иначе упор в контекст/таймаут и разваливается разбор нумерованного списка; частичный
  результат сохраняется даже при ошибке/отмене. Отмена (aiCancel) проверяется между порциями.
  parseAiResponseAndApply парсит нумерованный ответ строго по возрастанию (с «1.»), склеивая
  многострочные; при недоборе строк копится missing и показывается предупреждение о сдвиге.
  Применение чистит прилипший ведущий префикс-эхо (stripLeadingPrefix), не трогая легитимный
  ведущий `[var]`. includeSpeaker добавляет `[Speaker]` как контекст (промпт запрещает
  включать в перевод). Прогресс двигается только при сохранении в БД — тут лишь editorDirty=true.
  **Жалоба «промт следует только первой пачке» — по коду НЕ воспроизводится (аудит):**
  `runChunked` вызывает `buildSystem(chunk.length)` и шлёт ПОЛНЫЙ system-промт в КАЖДОЙ пачке —
  и для Ollama (`/api/generate`, поле `system`), и для API (`llm_chat_request` → `messages[system]`).
  Каждая пачка — независимый stateless-запрос с полными инструкциями. Значит жалоба либо
  устранена рефактором на чанки, либо про иное (ручной режим / поведение модели тестера) —
  **уточнить у тестера перед закрытием.**

## `PairsWidget.vue`
- **Роль:** виджет рабочих пространств — карточки пар языков со статусом, прогрессом и меню
  экспорта; на дашборде.
- **Состояние:** из стора translationPairs; локально exportMenuFor.
- **Логика:** switchPair, deletePair, exportTranslation, removeMod, exportAllStrings. Клик по
  карточке переключает активную пару; меню экспорта по кнопке.
- **Нюансы:** статус пары (`statusKey`): черновик (draft) / собрано (built) / изменено (dirty
  — собрано, но БД менялась). Меню экспорта закрывается кликом вне (глобальный слушатель).
  is_legacy-пара (без source/target, без удаления). doExportStrings при неактивной паре
  сначала переключается на неё (экспорт читает БД активной пары). Экспорт full/mod и «удалить
  мод» — только для is_built.

## `FilesModal.vue`
- **Роль:** модалка выбора исходного файла игры (не только извлечённых) для открытия в редакторе.
- **Состояние:** files, loading, query, filter (all/empty/extracted/lang). Computed counts,
  visible.
- **Логика:** invoke list_game_files. Клик → закрывает модалку, openEditor(rel_path,
  {silent:true}), затем открывает SourceViewer.
- **Нюансы:** статусы файла — extracted (с прогрессом translated/total), lang (готовый
  языковой файл, показывает код), empty; фильтрация и счётчики по этим статусам.

## `UncoveredModal.vue`
- **Роль:** отчёт диагностики покрытия (roadmap 0.1) — строки, замеченные в игре, но не покрытые
  переводом. Самодостаточные scoped-стили (`uc-*`), т.к. `.fm-*` FilesModal — scoped.
- **Состояние:** entries, loading, query, showAll (false = только кандидаты `in_db=false`).
  Computed candidateCount, visible (фильтр + поиск).
- **Логика:** onMounted → invoke `read_uncovered` (бэкенд сверяет лог с БД). `refresh` перечитывает,
  `doClear` → `clear_uncovered` (сброс лога). `add(e)` → `addManualString(text,'',type,false)`
  (say→dialogue, иначе ui), помечает `in_db=true` (уходит из «кандидатов») + тост `uncov_added`.
- **Нюансы:** фильтр по умолчанию «кандидаты» = видно в игре, но нет в базе (извлечение не поймало);
  вкладка «всё» показывает и `in_db`-строки (бейдж «в базе»). Требует диагностической сборки +
  прогона игры, чтобы лог наполнился. Иконки search/x/undo/trash/plus. Открывается из Dashboard.

## `AddStringModal.vue`
- **Роль:** добавление/редактирование ручной строки перевода.
- **Состояние:** type (dialogue/ui), original, translation, position, toCurrent, addedCount.
  Из стора manualEditTarget, currentFilePath/MANUAL_FILE, scrollToBlock.
- **Логика:** submit — при правке updateManualString, при добавлении addManualString(original,
  translation, type, toCurrent, position); после добавления поле чистится, фокус
  возвращается, счётчик растёт, скролл к новому блоку (пакетный ввод без закрытия окна).
- **Нюансы:** editMode по наличию manualEditTarget; при правке предзаполняется, тип из
  block_type. Выбор «куда добавить» (в текущий файл на позицию / в отдельный файл ручных
  строк) — только при добавлении и если открыт реальный файл (не MANUAL_FILE). Ctrl+Enter =
  submit.

## `DeliveryHooksModal.vue`
- **Роль:** редактор хуков доставки — Python-сниппеты, вплетаемые в мод (монкипатчи движка
  через API RenForge).
- **Состояние:** hooks (массив {name, phase init/early, enabled, code, scope global/project}),
  status, busy, apiOpen.
- **Логика:** invoke get_delivery_hooks, validate_delivery_hook (по каждому включённому
  непустому), save_delivery_hooks. Шаблоны TEMPLATES (wrap/filter/patch), перемещение/удаление,
  сворачиваемая справка по API (renforge_tr, renforge_wrap, renforge_filter, renforge_add).
- **Нюансы:** «Проверить» и «Сохранить» валидируют весь набор; сохранение блокируется при
  ошибке и показывает имя+сообщение проблемного хука. Payload нормализуется (phase init/early,
  scope global/project).

## `TmModal.vue`
- **Роль:** управление памятью переводов (TM) — таблица с поиском, пагинацией, CRUD.
- **Состояние:** entries, total, query, page (50/стр), showAdd, add.
- **Логика:** invoke tm_list (query/limit/offset), tm_upsert, tm_delete, tm_clear. Поиск
  дебаунсится (250мс), сбрасывает страницу. clearAll — через ask.
- **Нюансы:** ключ строки таблицы — `target_lang + \u0001 + original` (составной). Правка
  сохраняется на @change (saveEntry). При удалении последней записи на странице — откат на
  предыдущую. Столбец hits — счётчик срабатываний.

## `AccentPicker.vue`
- **Роль:** выбор акцентного цвета UI — пресеты-свотчи + кастомный HEX/нативная палитра.
- **Логика:** pick(key) пресет, pickHex(v) — валидирует `#RRGGBB`; всё в localStorage.
- **Нюансы:** нативный `input[type=color]` спрятан 0×0 (в этом WebView он рисует свой свотч
  при любых стилях), палитра открывается программно colorInput.click(); инпут у левого
  нижнего угла, чтобы попап лёг рядом.

## `GlobalMessages.vue`
- Стек тост-уведомлений (success/error/warn). Рендерит toasts из стора, removeToast, иконка
  по типу; у липкого тоста экспорта при isExporting — кнопка отмены (cancelExport). Анимация
  transition-group, уважает prefers-reduced-motion.

## Мелкие
- **`Icon.vue`** — SVG-иконки одним компонентом: пропсы name/size/strokeWidth, viewBox 0 0 24
  24, путь по name (eye/gear/trash/translate/volume/volume-x/alert/…).
- **`EmptyState.vue`** — заглушка пустого состояния: пропсы icon/title/hint + слот.
- **`AboutModal.vue`** — «О программе»: версия, GPL-3.0, GitHub, атрибуция. invoke
  read_text_file (licenses/THIRD_PARTY_NOTICES.txt через resolveResource) с англ. фоллбэком;
  open_in_explorer открывает папку лицензий.
- **`UpdateModal.vue`** — миграция переводов на новую версию игры: выбор старой папки, запуск
  migrateTranslations, отчёт (перенесено точно/нечётко, новые/исчезнувшие, ещё не переведено).
