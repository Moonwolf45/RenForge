# Экстрактор (Python-сайдкар, src-tauri/tools)

Сайдкар `rpyc_extractor` (PyInstaller-бинарь в `bin/`). Собирается spec-файлом
`tools/extractor/rpyc_extractor.spec`, вендорит unrpyc (копия .py-дерева в бандл под
`unrpyc/`, добавляется в sys.path). **После правок main.py/unpickler.py/unrpyc нужно
пересобрать сайдкар** venv-питоном (`extractor/venv/Scripts/python.exe -m PyInstaller
--clean --noconfirm rpyc_extractor.spec`) и скопировать `dist/rpyc_extractor.exe` в
`bin/rpyc_extractor-x86_64-pc-windows-msvc.exe`.

## main.py — CLI, извлечение, экспорт метода

### Режимы CLI
- `--dir <game> --out <json> [--source-lang auto|original|EN|...]` — полное извлечение в JSON.
- `--dir <game> --list-languages` — быстрый список языков-источников (печатает
  `RENFORGE_LANGS:<json>`), без извлечения. Для селектора «Переводить с».
- `--decompile <rpyc> --out <rpy>` — декомпиляция одного файла (читалка).
- `--check-syntax <py>` — проверка синтаксиса хука доставки (`compile()`), 0/1.

### Ключевые функции
- **`process_directory(input_dir, output_file, source_lang)`** — оркестратор извлечения:
  собирает файлы, гоняет AST или regex-фоллбэк, добавляет движковые common-строки, пишет JSON
  с метаданными (project_name/is_legacy_format/available_languages/source_language/game_name/
  game_version/engine_version/strings/**skipped_files**). `skipped_files` (roadmap 1.3) — пути
  относительно `input_dir` файлов, которые не удалось разобрать: и сбой `explore_ast` (Exception
  на узле), и пустой `load_ast` (падало молча, теперь тоже считается). Прокидывается в
  `ExtractedData` (Rust) → `ExtractResult` команды `extract_and_ingest_project` → тост на фронте.
- **`detect_engine_version(input_dir)`** — версия движка Ren'Py из `<root>/renpy/vc_version.py`
  (фоллбэк `renpy/__init__.py`), регексп `version = "X.Y.Z"`.
- **Мета игры (`GAME_META` + `capture_game_meta`, в unpickler.py):** имя/версия собираются из
  Define/Default-узлов при обходе AST — `config.name`/`gui.name`→name, `config.version`→version.
  Голая переменная `name` кладётся в `name_fb` и принимается как заголовок ТОЛЬКО если она
  многословна или ≥12 символов (иначе может быть именем персонажа). Резолв — в `process_directory`.
- **`collect_rpyc_files(input_dir, source_lang)`** — выбор файлов по языку-источнику.
  ПРИОРИТЕТ: папки `tl/<lang>/` проверяются ПЕРВЫМИ (иначе имя языка вроде "english" сматчило
  бы суффиксную ветку). Иначе: auto → EN если есть, иначе оригинал; original → без суффикса;
  код/имя → по суффиксу. Пропуск tl/cache.
- **`scan_available_languages` / `_scan_tl_languages` / `detect_suffix`** — доступные языки
  (суффиксы `_XX` + папки tl/). detect_suffix регистронезависим, длинные суффиксы (PT-BR,
  ZH-HANT) первыми.
- **`parse_rpy_file(filepath, results)`** — **regex-фоллбэк** по тексту .rpy, когда .rpyc
  нет (игра с исходниками). Ловит диалоги (`char "text"` / `"text"`), меню, define Character,
  UI `_("text")`. Грубее AST. Кодировки: utf-8-sig → cp1251.
- **`extract_engine_common(game_dir, existing_texts)`** — игроцентричные движковые строки из
  `renpy/common/*.rpy` (Сохранить/Выход/даты/скип), которых нет в game/. Регексп `_("...")`
  (НЕ `__()`), чёрный список внутренних файлов (ENGINE_COMMON_BLACKLIST: 00console/00developer
  /…), дедуп, без пересечения с уже извлечённым. who=`[ENGINE]`, source=`regex`. Только при
  source=original.
- **`attach_alt_texts(input_dir, primary_files, primary_strings)`** — multi-key delivery:
  собирает `alt_texts` (иные варианты текста той же строки по translation id) из
  ОДНОЯЗЫЧНЫХ сиблинг-источников (base + tl/<same-lang>). Одноязычность — по доле идентичных
  строк на общих id (`SAME_LANG_THRESHOLD=0.5`, сэмпл-детект через `_extract_id_text_map`
  с limit; нормализация trim). Вызывается в AST-ветке в try/except (не ломает извлечение).
  Разноязычные tl (Spanish/…) отвергаются (доля идентичных ~0).
- **`_extract_id_text_map(files, limit=None)`** — строит `{translation_id: text}` по набору
  .rpyc (тем же explore_ast); limit — для дешёвого сэмпла.
- **`decompile_file(rpyc_path, out_path)`** — декомпиляция через вендоренный unrpyc
  (`init_offset=True`), пишет **только в out_path** (кэш), рядом с .rpyc ничего не создаёт
  (иначе двойная загрузка модуля движком). `_unrpyc_dirs()` — пути к unrpyc в бандле и dev-дереве.

### Метка способа извлечения (`source`)
- AST-ветка: после цикла `explore_ast` — `s.setdefault("source", "ast")`.
- regex-фоллбэк: после `parse_rpy_file` — `s.setdefault("source", "regex")`.
- engine common: дикт уже несёт `"source": "regex"`.
Далее пробрасывается в БД (колонка source) и тег AST/Regex в редакторе.

## unpickler.py — легаси-загрузчик AST

- **`RenpyUnpickler(pickle.Unpickler, encoding='latin1')`** — грузит пиклы Ren'Py.
  **`latin1` (не ASCII!):** биективно отображает любой байт 0x00–0xFF, никогда не падает на
  не-ASCII (старые игры Ren'Py 6.x, Python2-str в коротких binstring). Реальная перекодировка
  строк — в `sanitize_string` (latin1→cp1252/utf-8 по эвристике). Юникод-текст (BINUNICODE/
  UTF-8) от encoding не зависит. **Тот же принцип продублирован в вендоренном unrpyc** для
  читалки (`decompiler/renpycompat.py` pickle_safe_loads: ASCII→latin-1).
- **`load_ast(filepath) -> (tree, is_legacy)`** — читает .rpyc, извлекает pickle-данные,
  детект legacy (сырой zlib-скан vs заголовок RENPY RPC2), грузит через RenpyUnpickler.
- **`explore_ast(tree, results)`** — обход AST по типам узлов: Say (диалоги), Menu (меню),
  Translate, Screen + SL1, Show text, UI-элементы + SLUse, Python/Define/Default, layeredimage.
  Заполняет results диктами (type/id/file/line/who/what/prefix).
- **`extract_python_strings(code)` + `RenpyTranslationVisitor`** — тянут строки из Python-кода
  узлов: `_()`/`__()`/`Character`/`DynamicCharacter`/`Confirm`/`Notify`, промпт `renpy.input(...)`,
  и **легаси katawa-style `displayDict["lang"].key = "value"`** (visit_Assign; `name_XX` → код
  персонажа). Фоллбэк на регекс (FALLBACK_TRANSLATION_REGEX / LEGACY_DISPLAYDICT_REGEX /
  LEGACY_UI_CALL_REGEX / RENPY_INPUT_REGEX) при SyntaxError (Python2/легаси). NB: displayDict
  ИЗВЛЕКАЕТСЯ, но доставка на такие движки не реализована (08 — legacy-UI отложено): извлечение
  и доставка тут расходятся.
- **`clean_filename(path)`** — имя относительно `game/` (или basename). Берётся из
  `node.filename` — Ren'Py зашивает **имя исходника .rpy** внутрь .rpyc, поэтому в UI видим
  `.rpy`, а не `.rpyc` (логическое имя, не файл на диске).
- **`generate_real_renpy_id(current_label, filename, node, pending_prefix)`** — id строки:
  translation_identifier/identifier из узла, иначе `<label|filename>_<md5[:8]>` с точным
  экранированием seed (`\r\n`).
- **who/prefix:** who — говорящий/тег роли; prefix — ведущий оператор (напр. `voice "x.ogg"`),
  источник маппинга аудио в Rust.

### Аудит-проверка: дубли функций ОПРОВЕРГНУТЫ
Прежняя заметка подозревала, что `explore_ast` и `extract_layeredimage_strings` в
`unpickler.py` определены дважды. **Проверено прямым чтением + grep:** каждая определена
РОВНО ОДИН раз (`extract_layeredimage_strings` — строка 564, `explore_ast` — строка 594).
Дублей нет. Подозрение было артефактом наблюдения сабагента (explore_ast рекурсивна и
вызывается во множестве мест — вызовы приняли за определения). Пункт закрыт.

### Мёртвый код — УДАЛЁН
- `extract_targeted_use_args` (была «ФИКС №3», unpickler.py) — не вызывалась нигде, вытеснена
  инлайновой обработкой `SLUse` в `explore_ast`.
- Лишний импорт `clean_filename` в `main.py` — удалён (нужен только внутри `unpickler.py`).

### Старое (для истории, уже неактуально)
- **`extract_targeted_use_args(expr)` (помечена «ФИКС №3», unpickler.py ~496)** — определена,
  но НИГДЕ не вызывается. Вытеснена инлайновой обработкой `SLUse` в `explore_ast`
  (через `extract_python_strings` + `extract_implicit_string`). Безопасно удалить.
- **`clean_filename` импортируется в main.py (строка 9), но там не используется** (нужна
  только внутри unpickler.py) — лишний импорт.

## Вендоренные инструменты
- **`unrpyc/`** — декомпилятор .rpyc (читалка, `--decompile`). Правка: latin-1 в
  `decompiler/renpycompat.py`.
- **`unrpa/`** — распаковщик .rpa (сайдкар `unrpa`, собирается `tools/unrpa.spec` /
  `build_unrpa.py`).
