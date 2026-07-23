import argparse
import json
import sys
import os
import re
import hashlib

# Импортируем инструменты из нашего второго файла
from unpickler import load_ast, explore_ast, GAME_META, clean_filename


def _unrpyc_dirs():
    """Возможные расположения вендоренного unrpyc: в бандле (_MEIPASS/unrpyc) и в dev-дереве."""
    dirs = []
    mei = getattr(sys, "_MEIPASS", None)
    if mei:
        dirs.append(os.path.join(mei, "unrpyc"))
    here = os.path.dirname(os.path.abspath(__file__))
    # tools/extractor -> ../unrpyc (соседний каталог в tools/)
    dirs.append(os.path.normpath(os.path.join(here, "..", "unrpyc")))
    return dirs


def decompile_file(rpyc_path, out_path):
    """Декомпилирует один .rpyc через вендоренный unrpyc и пишет исходник в out_path.
    Только чтение исходной игры: результат идёт в наш кэш, рядом с .rpyc НИЧЕГО не создаётся
    (иначе словили бы двойную загрузку модуля движком)."""
    import io as _io
    for d in _unrpyc_dirs():
        if os.path.isdir(d) and d not in sys.path:
            sys.path.insert(0, d)
    try:
        import unrpyc
        import decompiler
    except Exception as e:
        print("DECOMPILE_ERROR: unrpyc not available: %r" % e, file=sys.stderr)
        sys.exit(3)

    from pathlib import Path
    ctx = unrpyc.Context()
    ast = unrpyc.get_ast(Path(rpyc_path), False, ctx)
    buf = _io.StringIO()
    try:
        options = decompiler.Options(log=[], init_offset=True)
    except TypeError:
        options = decompiler.Options()
    decompiler.pprint(buf, ast, options)
    src = buf.getvalue()
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(src)
    print("DECOMPILE_OK: %d chars -> %s" % (len(src), out_path))


def detect_engine_version(input_dir):
    """Версия движка Ren'Py из <root>/renpy/vc_version.py (X.Y.Z)."""
    root = os.path.dirname(os.path.normpath(input_dir))  # input_dir = .../game
    for cand in (os.path.join(root, 'renpy', 'vc_version.py'),
                 os.path.join(root, 'renpy', '__init__.py')):
        try:
            with open(cand, 'r', encoding='utf-8', errors='replace') as f:
                txt = f.read()
            m = re.search(r"version\s*=\s*['\"](\d+\.\d+(?:\.\d+)?)", txt)
            if m:
                return m.group(1)
        except Exception:
            pass
    return None

# Известные суффиксы языков для legacy Ren'Py (файлы вида script_XX.rpyc)
LANG_SUFFIXES = {
    'DE': 'german', 'ES': 'spanish', 'FR': 'french', 'JP': 'japanese', 
    'KR': 'korean', 'PL': 'polish', 'PT-BR': 'portuguese_br', 'RU': 'russian',
    'ZH-HANT': 'chinese_traditional', 'ZH': 'chinese', 'IT': 'italian', 
    'NL': 'dutch', 'SV': 'swedish', 'CS': 'czech', 'HU': 'hungarian', 
    'TR': 'turkish', 'AR': 'arabic', 'TH': 'thai', 'VI': 'vietnamese', 
    'ID': 'indonesian', 'UK': 'ukrainian', 'RO': 'romanian', 'BG': 'bulgarian', 
    'HR': 'croatian', 'EN': 'english'
}

def detect_suffix(filename):
    """Определяет языковой суффикс файла. Возвращает (base_name, suffix) или (filename, None).
    Сравнение регистронезависимо: script_ru / script_RU / script_Ru → суффикс 'RU'
    (реальные игры почти всегда пишут суффикс в нижнем регистре, напр. block1_ru.rpyc)."""
    name = filename[:-5] if filename.endswith('.rpyc') else filename  # убираем .rpyc
    name_up = name.upper()
    # Проверяем от длинных суффиксов к коротким (PT-BR, ZH-HANT перед ZH)
    for suffix in sorted(LANG_SUFFIXES.keys(), key=len, reverse=True):
        if name_up.endswith('_' + suffix):
            base = name[:-(len(suffix) + 1)]
            return base, suffix
    return name, None


def parse_rpy_file(filepath, results):
    """
    Парсит текстовый .rpy файл и извлекает переводимые строки через regex.
    Используется когда .rpyc файлов нет (игра распространяется с исходниками).
    """
    try:
        with open(filepath, 'r', encoding='utf-8-sig') as f:
            content = f.read()
    except UnicodeDecodeError:
        try:
            with open(filepath, 'r', encoding='cp1251') as f:
                content = f.read()
        except:
            return
    
    rel_path = os.path.basename(filepath)
    current_label = ""
    
    lines = content.split('\n')
    for line_num, line in enumerate(lines, 1):
        stripped = line.strip()
        
        # Пропускаем комментарии и пустые строки
        if not stripped or stripped.startswith('#'):
            continue
        
        # Определяем текущий label
        label_match = re.match(r'^label\s+(\w+)', stripped)
        if label_match:
            current_label = label_match.group(1)
            continue
        
        # --- ДИАЛОГИ: character "text" или "text" ---
        # Паттерн: опциональный идентификатор + строка в кавычках
        say_match = re.match(r'^(\w+)\s+"((?:[^"\\]|\\.)+)"', stripped)
        if say_match:
            who = say_match.group(1)
            what = say_match.group(2).replace('\\n', '\n').replace('\\"', '"')
            # Исключаем команды Ren'Py
            if who not in ('scene', 'show', 'hide', 'play', 'stop', 'queue', 'with', 
                          'jump', 'call', 'return', 'image', 'define', 'default',
                          'transform', 'style', 'screen', 'label', 'init', 'python',
                          'if', 'elif', 'else', 'while', 'for', 'pass', 'voice'):
                seed = f"{what}\r\n".encode('utf-8', errors='replace')
                say_id = f"{current_label}_{hashlib.md5(seed).hexdigest()[:8]}"
                results.append({
                    "type": "dialogue",
                    "id": say_id,
                    "file": rel_path,
                    "line": line_num,
                    "who": who,
                    "what": what,
                    "prefix": None
                })
            continue
        
        # Нарратор: просто "text" (без имени персонажа)
        narrator_match = re.match(r'^"((?:[^"\\]|\\.)+)"', stripped)
        if narrator_match:
            what = narrator_match.group(1).replace('\\n', '\n').replace('\\"', '"')
            seed = f"{what}\r\n".encode('utf-8', errors='replace')
            say_id = f"{current_label}_{hashlib.md5(seed).hexdigest()[:8]}"
            results.append({
                "type": "dialogue",
                "id": say_id,
                "file": rel_path,
                "line": line_num,
                "who": None,
                "what": what,
                "prefix": None
            })
            continue
        
        # --- МЕНЮ ---
        # Пункты меню: "text" с отступом после menu:
        if re.match(r'^"((?:[^"\\]|\\.)+)"(\s*if\s+.+)?:', stripped):
            menu_match = re.match(r'^"((?:[^"\\]|\\.)+)"', stripped)
            if menu_match:
                what = menu_match.group(1).replace('\\n', '\n').replace('\\"', '"')
                results.append({
                    "type": "menu",
                    "id": what,
                    "file": rel_path,
                    "line": line_num,
                    "who": "[ВЫБОР]",
                    "what": what,
                    "prefix": None
                })
            continue
        
        # --- CHARACTER DEFINITIONS ---
        define_match = re.match(r'^define\s+(\w+)\s*=\s*Character\s*\(\s*["\']([^"\']*)["\']', stripped)
        if define_match:
            code = define_match.group(1)
            name = define_match.group(2)
            results.append({
                "type": "python",
                "id": name,
                "file": rel_path,
                "line": line_num,
                "who": f"[DEFINE: {code}]",
                "what": name,
                "prefix": None
            })
            continue
        
        # --- UI СТРОКИ: _("text") ---
        for m in re.finditer(r'_\(\s*["\']([^"\']+)["\']\s*\)', stripped):
            text = m.group(1)
            results.append({
                "type": "ui",
                "id": text,
                "file": rel_path,
                "line": line_num,
                "who": "[ИНТЕРФЕЙС]",
                "what": text,
                "prefix": None
            })


def _scan_tl_languages(input_dir):
    """Языки из папок tl/<lang>/ (современный стандарт мультиязычных игр).
    Возвращает dict {lang_lower: abs_path}. Папка 'None' (дефолтный язык) пропускается."""
    tl_dirs = {}
    tl_root = os.path.join(input_dir, 'tl')
    if os.path.isdir(tl_root):
        for n in os.listdir(tl_root):
            p = os.path.join(tl_root, n)
            if not os.path.isdir(p) or n.lower() == 'none':
                continue
            # есть ли внутри переводы (.rpyc/.rpy)?
            has_files = False
            for _r, _d, _fs in os.walk(p):
                if any(f.endswith('.rpyc') or f.endswith('.rpy') for f in _fs):
                    has_files = True
                    break
            if has_files:
                tl_dirs[n.lower()] = p
    return tl_dirs


def scan_available_languages(input_dir):
    """Сканирует директорию и определяет доступные языки (для legacy-игр)."""
    found_suffixes = set()
    has_originals = False
    
    for root, dirs, files in os.walk(input_dir):
        rel_root = os.path.relpath(root, input_dir).replace('\\', '/')
        if rel_root.startswith('tl/') or rel_root.startswith('cache/') or '/tl/' in rel_root:
            continue
        for file in files:
            if file.endswith('.rpyc'):
                _, suffix = detect_suffix(file)
                if suffix:
                    found_suffixes.add(suffix)
                else:
                    has_originals = True
    
    languages = []
    if has_originals:
        languages.append("original")
    for suffix in sorted(found_suffixes):
        languages.append(suffix.lower())
    # Языки из tl/<lang>/ (могут переводиться как источник)
    for lang in sorted(_scan_tl_languages(input_dir).keys()):
        if lang not in languages:
            languages.append(lang)
    
    return languages


def collect_rpyc_files(input_dir, source_lang):
    """
    Собирает список .rpyc файлов для обработки на основе выбранного source language.
    
    source_lang:
      - "auto" → английский если есть, иначе оригинал
      - "original" → файлы без языкового суффикса
      - "EN", "DE", "RU" и т.д. → файлы с соответствующим суффиксом
    """
    available = scan_available_languages(input_dir)
    
    # Источник может указывать на папку tl/<lang>/ (современные мультиязычные игры).
    # Это проверяем ПЕРВЫМ — до суффиксной логики, т.к. имя языка (напр. "english")
    # совпало бы с LANG_SUFFIXES и ошибочно ушло бы в ветку суффиксов.
    tl_dirs = _scan_tl_languages(input_dir)
    if source_lang and source_lang.lower() in tl_dirs:
        tl_files = []
        for root, dirs, files in os.walk(tl_dirs[source_lang.lower()]):
            for file in files:
                if file.endswith('.rpyc'):
                    tl_files.append(os.path.join(root, file))
        return tl_files, source_lang.lower(), available
    
    # Определяем целевой язык
    if source_lang == "auto":
        if "en" in available:
            target_suffix = "EN"
        else:
            target_suffix = None  # оригинал
    elif source_lang.lower() == "original":
        target_suffix = None
    else:
        # Ищем суффикс по имени или коду
        target_suffix = None
        for code, name in LANG_SUFFIXES.items():
            if source_lang.upper() == code or source_lang.lower() == name:
                target_suffix = code
                break
        if target_suffix is None and source_lang.upper() in LANG_SUFFIXES:
            target_suffix = source_lang.upper()
    
    rpyc_files = []
    
    for root, dirs, files in os.walk(input_dir):
        rel_root = os.path.relpath(root, input_dir).replace('\\', '/')
        if rel_root.startswith('tl/') or rel_root.startswith('cache/') or '/tl/' in rel_root:
            continue
        
        for file in files:
            if not file.endswith('.rpyc'):
                continue
            
            _, file_suffix = detect_suffix(file)
            
            if target_suffix is None:
                # Берём только оригиналы (без суффикса)
                if file_suffix is None:
                    rpyc_files.append(os.path.join(root, file))
            else:
                # Берём только файлы с нужным суффиксом
                if file_suffix == target_suffix:
                    rpyc_files.append(os.path.join(root, file))
    
    return rpyc_files, target_suffix, available


# Файлы renpy/common, которые НЕ отдаём на перевод (внутренние/дев/конфиг — не видны
# игроку). Остальное (gui/save/preferences/accessibility/даты/скип…) — игроцентрично.
ENGINE_COMMON_BLACKLIST = {
    "00console", "00director", "00developer", "00inspector", "00build", "00updater",
    "00gamepad", "00iap", "00gltest", "00performance", "00shaders", "00compat",
    "00obsolete", "00keymap", "00gamekeymap", "00style", "00stylepreferences",
    "00themes", "00debug", "00gallery",
}

def extract_engine_common(game_dir, existing_texts):
    """Извлекает игроцентричные строки самого движка Ren'Py из renpy/common (вне game/):
    Сохранить/Загрузить/Выход, названия дней/месяцев, подтверждения скипа и т.п. Игрок их
    видит, но сканированием game/ они не берутся. Источник — _()-строки в .rpy-исходниках
    common (надёжно ловятся регекспом; .rpyc-AST берёт их хуже). Минус блок-лист внутренних
    файлов, дедуп, без пересечения с уже извлечённым из игры. __()-строки пропускаем."""
    parent = os.path.dirname(os.path.normpath(game_dir))
    common = os.path.join(parent, "renpy", "common")
    out = []
    if not os.path.isdir(common):
        return out
    # _("...") или _('...'), но НЕ __("...") (двойное подчёркивание = no-op перевод)
    rx = re.compile(r'(?<!\w)_\(\s*(["\'])((?:(?!\1).)+)\1\s*\)')
    seen = set()
    for fn in sorted(os.listdir(common)):
        if not fn.endswith(".rpy"):
            continue
        if fn.rsplit(".", 1)[0] in ENGINE_COMMON_BLACKLIST:
            continue
        try:
            text = open(os.path.join(common, fn), "r", encoding="utf-8", errors="replace").read()
        except Exception:
            continue
        for m in rx.finditer(text):
            what = m.group(2)
            if not what or not what.strip():
                continue
            if what in seen or what in existing_texts:
                continue
            seen.add(what)
            seed = (what + "\r\n").encode("utf-8", "replace")
            out.append({
                "type": "ui",
                "id": "engine_" + hashlib.md5(seed).hexdigest()[:10],
                "file": "engine (renpy common)",
                "line": 0,
                "who": "[ENGINE]",
                "what": what,
                "prefix": None,
                "source": "regex",
            })
    return out


# === Multi-key delivery: alt-тексты из одноязычных источников ===
# Порог доли идентичных строк, при котором два источника считаются ОДНИМ языком
# (base-скрипт vs tl/<lang>, различающиеся лишь реформулировками). Ниже — разные языки.
SAME_LANG_THRESHOLD = 0.5


def _norm_txt(s):
    return (s or "").strip()


def _extract_id_text_map(files, limit=None):
    """Строит {translation_id: text} по набору .rpyc через тот же AST-обход.
    limit — обработать не более N файлов (для дешёвого сэмпл-детекта одноязычности)."""
    m = {}
    for i, fp in enumerate(files):
        if limit is not None and i >= limit:
            break
        try:
            tree, _is_legacy = load_ast(fp)
        except Exception:
            continue
        if not tree:
            continue
        tmp = []
        try:
            explore_ast(tree, results=tmp)
        except Exception:
            continue
        for s in tmp:
            sid = s.get("id")
            what = s.get("what")
            if sid and what is not None and sid not in m:
                m[sid] = what
    return m


def attach_alt_texts(input_dir, primary_files, primary_strings):
    """Дописывает к строкам alt_texts — иные текстовые варианты той же строки (по
    translation id) из ОДНОЯЗЫЧНЫХ сиблинг-источников (напр. base English + tl/english,
    различающиеся реформулировками). Доставка регистрирует перевод под всеми вариантами,
    поэтому строка матчится независимо от того, какой текст показан в рантайме.

    Одноязычность определяется по доле идентичных строк на общих id (сэмпл), без определения
    языка: реворд-overlay сохраняет большинство строк идентичными (доля высокая), другой
    язык почти не даёт совпадений (доля ~0)."""
    prim = {}
    for s in primary_strings:
        sid = s.get("id")
        if sid and sid not in prim:
            prim[sid] = _norm_txt(s.get("what"))
    if not prim:
        return

    primary_set = set(primary_files)
    same_lang_maps = []
    for cand in scan_available_languages(input_dir):
        try:
            cand_files = collect_rpyc_files(input_dir, cand)[0]
        except Exception:
            continue
        if not cand_files or set(cand_files) == primary_set:
            continue  # тот же источник, что и основной — пропускаем
        # Сэмпл-детект одноязычности (дёшево: несколько файлов).
        sample = _extract_id_text_map(cand_files, limit=5)
        shared = [sid for sid in sample if sid in prim]
        if len(shared) < 10:
            sample = _extract_id_text_map(cand_files, limit=25)
            shared = [sid for sid in sample if sid in prim]
        if not shared:
            continue
        identical = sum(1 for sid in shared if _norm_txt(sample[sid]) == prim[sid])
        frac = identical / len(shared)
        if frac >= SAME_LANG_THRESHOLD:
            # Одноязычный источник — полный сбор его id->text.
            same_lang_maps.append(_extract_id_text_map(cand_files))
            print(f"[INFO] Alt-источник '{cand}': одноязычный (идентичных {frac:.0%}), +alt-ключи")

    if not same_lang_maps:
        return

    tagged = 0
    for s in primary_strings:
        sid = s.get("id")
        if not sid:
            continue
        prim_txt = _norm_txt(s.get("what"))
        alts = []
        for m in same_lang_maps:
            cand_txt = m.get(sid)
            if cand_txt is None:
                continue
            if _norm_txt(cand_txt) == prim_txt:
                continue  # идентичен основному — доставлять как alt незачем
            if cand_txt not in alts:
                alts.append(cand_txt)
        if alts:
            s["alt_texts"] = alts
            tagged += 1
    if tagged:
        print(f"[INFO] Alt-ключи проставлены для {tagged} строк (multi-key delivery)")


def process_directory(input_dir: str, output_file: str, source_lang: str = "auto"):
    print(f"[INFO] Сканируем директорию: {input_dir}")
    print(f"[INFO] Исходный язык: {source_lang}")
    
    # Определяем доступные языки
    available_languages = scan_available_languages(input_dir)
    print(f"[INFO] Доступные языки: {available_languages}")
    
    # Собираем файлы для обработки
    rpyc_files, used_suffix, _ = collect_rpyc_files(input_dir, source_lang)
    
    actual_source = LANG_SUFFIXES.get(used_suffix, used_suffix) if used_suffix else "original"
    print(f"[INFO] Выбран источник: {actual_source} ({len(rpyc_files)} файлов)")
    
    extracted_data = {
        "project_name": os.path.basename(os.path.normpath(input_dir)),
        "is_legacy_format": False,
        "available_languages": available_languages,
        "source_language": actual_source,
        "game_name": None,
        "game_version": None,
        "engine_version": detect_engine_version(input_dir),
        "strings": []
    }
    GAME_META.clear()
    
    if not rpyc_files:
        # Фоллбэк: если .rpyc нет, парсим .rpy файлы напрямую
        rpy_files = []
        for root, dirs, files in os.walk(input_dir):
            rel_root = os.path.relpath(root, input_dir).replace('\\', '/')
            if rel_root.startswith('tl/') or rel_root.startswith('cache/') or '/tl/' in rel_root:
                continue
            for file in files:
                if file.endswith('.rpy'):
                    rpy_files.append(os.path.join(root, file))
        
        if rpy_files:
            print(f"[INFO] .rpyc не найдены, парсим {len(rpy_files)} файлов .rpy напрямую")
            for filepath in rpy_files:
                parse_rpy_file(filepath, extracted_data["strings"])
            # Метка способа извлечения: regex-парсер по тексту .rpy (менее надёжен, чем AST).
            for s in extracted_data["strings"]:
                s.setdefault("source", "regex")
        else:
            print(f"[WARNING] Ни .rpyc, ни .rpy файлы не найдены!")
    else:
        skipped_files = []
        for filepath in rpyc_files:
            tree, is_legacy = load_ast(filepath)
            if is_legacy:
                extracted_data["is_legacy_format"] = True
            if tree:
                # Изоляция ошибок: сбой explore_ast на ОДНОМ файле (нестандартный узел,
                # RecursionError и т.п.) не должен ронять всё извлечение (иначе теряется
                # ВЕСЬ результат — строки пишутся в JSON только после цикла). Копим во
                # временный буфер и вливаем в общий список только при успехе — на валидных
                # файлах порядок и содержимое вывода не меняются.
                tmp = []
                try:
                    explore_ast(tree, results=tmp)
                    extracted_data["strings"].extend(tmp)
                except Exception as e:
                    skipped_files.append(filepath)
                    print(f"[WARN] Пропущен файл (сбой разбора AST): {filepath} -> {e!r}", file=sys.stderr)
                    continue
        if skipped_files:
            print(f"[WARN] Пропущено файлов из-за ошибок разбора AST: {len(skipped_files)}", file=sys.stderr)
        # Метка способа извлечения: AST скомпилированного .rpyc (надёжный путь).
        for s in extracted_data["strings"]:
            s.setdefault("source", "ast")
        # Multi-key delivery: alt-тексты из одноязычных источников (base + tl/<same>).
        # Обёрнуто в try/except — детект alt-ключей никогда не должен ломать извлечение.
        try:
            attach_alt_texts(input_dir, rpyc_files, extracted_data["strings"])
        except Exception as e:
            print(f"[WARN] Детект alt-ключей не удался: {e!r}", file=sys.stderr)

    # Игроцентричные строки самого движка из renpy/common (Сохранить/Выход/даты/скип…),
    # которых нет в game/. Извлекаем только при source=original (для перевода с оригинала);
    # на переводном источнике движковый UI не из game/ не нужен.
    if actual_source == "original":
        existing_texts = set((s.get("what") or "").strip() for s in extracted_data["strings"])
        engine_common = extract_engine_common(input_dir, existing_texts)
        if engine_common:
            extracted_data["strings"].extend(engine_common)
            print(f"[INFO] Движковые common-строки (renpy/common): +{len(engine_common)}")

    # Имя/версия игры, собранные из Define-узлов
    _name = GAME_META.get("name")
    if not _name:
        _fb = GAME_META.get("name_fb")
        # bare-переменную name принимаем как заголовок, только если она похожа на
        # название (многословное/длинное) — иначе это может быть имя персонажа
        if _fb and (" " in _fb or len(_fb) >= 12):
            _name = _fb
    extracted_data["game_name"] = _name
    extracted_data["game_version"] = GAME_META.get("version") or GAME_META.get("version_fb")
    
    try:
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump(extracted_data, f, ensure_ascii=False, indent=4)
            
        print(f"\n[SUCCESS] Обработано строк: {len(extracted_data['strings'])}")
        print(f"[SUCCESS] Легаси режим (старый Ren'Py): {'ДА' if extracted_data['is_legacy_format'] else 'НЕТ'}")
        print(f"[SUCCESS] Данные сохранены в {output_file}")
    except Exception as e:
        print(f"[ERROR] Ошибка записи: {e}", file=sys.stderr)
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="RenForge AST Extractor (Python Sidecar)")
    parser.add_argument("--dir", required=False, help="Путь к папке game/ (где лежат .rpyc)")
    parser.add_argument("--out", required=False, help="Путь для сохранения .json файла")
    parser.add_argument("--source-lang", default="auto", 
                       help="Исходный язык для извлечения (auto/original/EN/DE/RU/...)")
    parser.add_argument("--list-languages", action="store_true",
                       help="Быстро вывести JSON доступных языков игры и выйти (без извлечения)")
    parser.add_argument("--decompile", help="Путь к .rpyc для декомпиляции (через unrpyc) в --out")
    parser.add_argument("--check-syntax", dest="check_syntax",
                       help="Проверить синтаксис Python-файла (compile) и выйти. 0 = ок, 1 = ошибка.")
    
    args = parser.parse_args()

    # Режим проверки синтаксиса пользовательского хука доставки (экспертный режим).
    if args.check_syntax:
        try:
            with open(args.check_syntax, "r", encoding="utf-8") as f:
                src = f.read()
            compile(src, "<renforge_hook>", "exec")
            print("OK")
            return
        except SyntaxError as e:
            print(f"Строка {e.lineno}: {e.msg}", file=sys.stderr)
            sys.exit(1)
        except Exception as e:
            print(str(e), file=sys.stderr)
            sys.exit(1)

    # Режим декомпиляции одного файла (экспертный просмотр исходника). Не требует --dir.
    if args.decompile:
        if not args.out:
            print("[ERROR] --out обязателен для --decompile", file=sys.stderr)
            sys.exit(2)
        if not os.path.isfile(args.decompile):
            print(f"[ERROR] Файл не найден: {args.decompile}", file=sys.stderr)
            sys.exit(1)
        decompile_file(args.decompile, args.out)
        return
    
    if not args.dir or not os.path.isdir(args.dir):
        print(f"[ERROR] Директория не найдена: {args.dir}", file=sys.stderr)
        sys.exit(1)

    # Быстрый режим: только список языков (без анпиклинга/распаковки) — для UI-селектора
    # «Переводить с» ДО запуска полного извлечения.
    if args.list_languages:
        langs = scan_available_languages(args.dir)
        # Печатаем чистый JSON отдельной строкой-маркером, чтобы Rust надёжно распарсил
        # его независимо от прочих [INFO]-строк.
        print("RENFORGE_LANGS:" + json.dumps(langs, ensure_ascii=False))
        return

    if not args.out:
        print("[ERROR] --out обязателен (кроме режима --list-languages)", file=sys.stderr)
        sys.exit(2)

    process_directory(args.dir, args.out, args.source_lang)


if __name__ == "__main__":
    main()
