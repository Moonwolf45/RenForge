import pickle
import zlib
import struct
import io
import sys
import hashlib
import os
import ast as python_ast
import re

_mock_classes = {}

def get_mock_class(module_name, class_name):
    key = (module_name, class_name)
    if key not in _mock_classes:
        class MockNode:
            __module__ = module_name
            __class_name__ = class_name
            
            def __init__(self, *args, **kwargs):
                pass

            def __setstate__(self, state):
                if isinstance(state, dict):
                    self.__dict__.update(state)
                elif isinstance(state, tuple):
                    if len(state) == 2:
                        dict_state, slot_state = state
                        if isinstance(dict_state, dict):
                            self.__dict__.update(dict_state)
                        if isinstance(slot_state, dict):
                            self.__dict__.update(slot_state)
                        elif not isinstance(dict_state, dict) and not isinstance(slot_state, dict):
                            self.__dict__['_custom_tuple_state'] = state
                    else:
                        self.__dict__['_custom_tuple_state'] = state
                else:
                    self.__dict__['_raw_state'] = state

            def __setitem__(self, key, value):
                self.__dict__[key] = value
                
            def __getitem__(self, item):
                return self.__dict__.get(item)
                
            def append(self, item):
                if not hasattr(self, '_appended_items'):
                    self._appended_items = []
                self._appended_items.append(item)

            def __repr__(self):
                return f"<{self.__module__}.{self.__class_name__}>"

        MockNode.__name__ = class_name
        _mock_classes[key] = MockNode
        
    return _mock_classes[key]

class DummyRevertableDict(dict):
    def __setstate__(self, state):
        if isinstance(state, dict):
            self.__dict__.update(state)

class DummyRevertableList(list):
    def __setstate__(self, state):
        if isinstance(state, dict):
            self.__dict__.update(state)

class DummyRevertableSet(set):
    def __setstate__(self, state):
        if isinstance(state, dict):
            self.__dict__.update(state)

# ТОЧЕЧНЫЙ ФИКС №1: Заставляем PyExpr (код Ren'Py) распаковываться как строки
# ВАЖНО: Pickle создаёт PyExpr дважды — первый раз с пустым значением (потом __setstate__
# получает реальные данные), второй раз с реальным значением. Поскольку str immutable,
# мы сохраняем source из __setstate__ в атрибут для последующего чтения.
class DummyPyExpr(str):
    def __new__(cls, value='', *args, **kwargs):
        if isinstance(value, bytes):
            try: value = value.decode('utf-8')
            except Exception: value = value.decode('latin1')
        return str.__new__(cls, value)
        
    def __setstate__(self, state):
        # state может быть:
        # 1) tuple: (version, source_string, (filename, line, source_again))  — старый формат
        # 2) tuple: (None, dict_with_metadata) — новый формат (уже имеет значение в str)
        # 3) dict: metadata
        if isinstance(state, tuple):
            if len(state) >= 2 and isinstance(state[1], str) and state[1]:
                # Старый формат: (version, source, location_tuple)
                # Сохраняем source для чтения, т.к. str(self) может быть пустым
                object.__setattr__(self, 'source', state[1])
            elif len(state) >= 2 and state[1] is not None and not isinstance(state[1], (str, dict)):
                # PyCode скрина (SL1, 6.12–6.17): state[1] — AST-модуль сгенерированного
                # кода скрина (ui.text/textbutton/...). Сохраняем для обхода в explore_ast.
                object.__setattr__(self, 'code_ast', state[1])
            elif len(state) == 2 and state[0] is None and isinstance(state[1], dict):
                # Новый формат: metadata dict, значение уже в str(self)
                pass

class RenpyUnpickler(pickle.Unpickler):
    def find_class(self, module, name):
        if name == 'RevertableDict': return DummyRevertableDict
        if name == 'RevertableList': return DummyRevertableList
        if name == 'RevertableSet': return DummyRevertableSet
        
        # Спасаем код от превращения в MockNode
        if name in ('PyExpr', 'PyCode'): return DummyPyExpr
        
        safe_modules = {'builtins', '__builtin__', 'collections', 'datetime'}
        
        if module in safe_modules:
            try:
                return super().find_class(module, name)
            except Exception:
                pass
                
        return get_mock_class(module, name)


def sanitize_string(s):
    if isinstance(s, str):
        if any(ord(c) > 0xff for c in s):
            return s
        try:
            raw_bytes = s.encode('latin1')
        except (AttributeError, UnicodeEncodeError):
            return s

        try:
            return raw_bytes.decode('utf-8')
        except UnicodeError:
            pass
        
        # CP1252 (Western European) — покрывает é, ñ, ü и т.д.
        # Приоритет выше азиатских, т.к. большинство VN на английском с вкраплениями диакритики
        try:
            decoded_1252 = raw_bytes.decode('cp1252')
            # Если все символы в диапазоне Latin (нет мусора) — это CP1252
            if all(ord(c) < 0x2000 for c in decoded_1252):
                return decoded_1252
        except UnicodeError:
            pass
        
        # Азиатские кодировки — только если результат реально содержит CJK символы
        for asian_enc in ('shift_jis', 'gbk', 'big5', 'euc_kr'):
            try:
                decoded = raw_bytes.decode(asian_enc)
                # Проверяем, что есть хотя бы один CJK символ
                has_cjk = any(ord(c) > 0x2E80 for c in decoded)
                if has_cjk:
                    return decoded
            except UnicodeError:
                continue

        # Фоллбэк: CP1252
        return raw_bytes.decode('cp1252', errors='replace')

    elif isinstance(s, bytes):
        try:
            return s.decode('utf-8')
        except UnicodeError:
            pass
        
        # CP1252 first (Western European)
        try:
            decoded_1252 = s.decode('cp1252')
            if all(ord(c) < 0x2000 for c in decoded_1252):
                return decoded_1252
        except UnicodeError:
            pass
        
        # Asian encodings only if CJK characters present
        for asian_enc in ('shift_jis', 'gbk', 'big5', 'euc_kr'):
            try:
                decoded = s.decode(asian_enc)
                if any(ord(c) > 0x2E80 for c in decoded):
                    return decoded
            except UnicodeError:
                continue
        
        return s.decode('cp1252', errors='replace')
            
    return s

def extract_pickle_data(filepath):
    with open(filepath, 'rb') as f:
        data = f.read()

    if data.startswith(b"RENPY RPC2"):
        idx = 10
        while idx + 12 <= len(data):
            slot_data = data[idx:idx+12]
            slot_id, start, length = struct.unpack("<III", slot_data)
            if slot_id == 0: 
                break
            if slot_id == 1:
                zlib_data = data[start:start+length]
                return zlib.decompress(zlib_data), False
            idx += 12
            
    offset = 0
    while True:
        idx = data.find(b'\x78\x9c', offset)
        if idx == -1:
            idx = data.find(b'\x78\xda', offset)
            
        if idx == -1:
            break
            
        try:
            decompressed = zlib.decompress(data[idx:])
            return decompressed, True
        except zlib.error:
            offset = idx + 2

    raise ValueError(f"Не найден zlib поток в файле: {filepath}")

def load_ast(filepath):
    try:
        pickle_data, is_legacy = extract_pickle_data(filepath)
    except Exception as e:
        print(f"[ERROR] Ошибка чтения {filepath}: {e}", file=sys.stderr)
        return None, False

    stream = io.BytesIO(pickle_data)
    unpickler = RenpyUnpickler(stream, encoding='latin1')
    
    try:
        tree = unpickler.load()
        return tree, is_legacy
    except Exception as e:
        print(f"[ERROR] Ошибка анпиклинга {filepath}: {e}", file=sys.stderr)
        return None, is_legacy


def clean_filename(path):
    path = path.replace('\\', '/')
    if 'game/' in path:
        return path.split('game/')[-1]
    return os.path.basename(path)


def get_arguments_code(arguments_node):
    if not arguments_node:
        return ""
    
    args_list = getattr(arguments_node, 'arguments', [])
    extrapos = getattr(arguments_node, 'extrapos', None)
    extrakw = getattr(arguments_node, 'extrakw', None)
    
    rv = []
    if args_list:
        for name, val in args_list:
            if name is not None:
                rv.append(f"{name}={val}")
            else:
                rv.append(str(val))
    if extrapos:
        rv.append("*" + str(extrapos))
    if extrakw:
        rv.append("**" + str(extrakw))
        
    if rv:
        return "(" + ", ".join(rv) + ")"
    return ""

def generate_real_renpy_id(current_label, filename, node, pending_prefix=None):
    raw_who = getattr(node, 'who', None)
    raw_what = getattr(node, 'what', '')
    attributes = getattr(node, 'attributes', None)
    temp_attributes = getattr(node, 'temporary_attributes', None)
    with_ = getattr(node, 'with_', None)
    arguments = getattr(node, 'arguments', None)

    rv = []
    if raw_who is not None:
        who_str = ""
        if isinstance(raw_who, str):
            who_str = raw_who.strip()
        elif hasattr(raw_who, 'name') and isinstance(raw_who.name, str):
            who_str = raw_who.name.strip()
        elif hasattr(raw_who, 'id') and isinstance(raw_who.id, str):
            who_str = raw_who.id.strip()
        else:
            who_str = str(raw_who).strip()
            
        if who_str:
            rv.append(sanitize_string(who_str))
            
    if attributes is not None:
        for attr in attributes:
            rv.append(sanitize_string(str(attr).strip()))
            
    if temp_attributes is not None:
        rv.append("@")
        for attr in temp_attributes:
            rv.append(sanitize_string(str(attr).strip()))
            
    what_str = sanitize_string(str(raw_what))
    what_str = what_str.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "").replace("\"", "\\\"")
    rv.append('"' + what_str + '"')
    
    if arguments is not None:
        arg_code = get_arguments_code(arguments)
        if arg_code:
            rv.append(sanitize_string(arg_code.strip()))
            
    if with_ is not None:
        rv.append("with")
        rv.append(sanitize_string(str(with_).strip()))
        
    res = " ".join(rv)
    
    if pending_prefix:
        seed_text = f"{pending_prefix}\r\n{res}\r\n"
    else:
        seed_text = f"{res}\r\n"
        
    seed = seed_text.encode('utf-8', errors='replace')
    md5_hash = hashlib.md5(seed).hexdigest()[:8]
    
    current_label = sanitize_string(current_label)
    if current_label:
        label_part = str(current_label).strip()
    else:
        label_part = filename.split('.')[0].replace('/', '_').replace('\\', '_')
        
    return f"{label_part}_{md5_hash}"

# =====================================================================
# ПАРСЕР ИНТЕРФЕЙСА (ТОЧЕЧНЫЙ AST ВИЗИТОР)
# =====================================================================
def get_ast_string(node):
    if hasattr(node, 'value') and isinstance(node.value, str):
        return node.value
    elif hasattr(node, 's') and isinstance(node.s, str):
        return node.s
    return None

class RenpyTranslationVisitor(python_ast.NodeVisitor):
    def __init__(self):
        self.strings = []
        self.char_definitions = []  # [(code, name), ...]

    def visit_Call(self, node):
        func_name = None
        if isinstance(node.func, python_ast.Name):
            func_name = node.func.id
        elif isinstance(node.func, python_ast.Attribute):
            func_name = node.func.attr

        # ТОЧЕЧНЫЙ ФИКС №2: Ловим вызовы Character для извлечения имен
        if func_name in ('_', '__', 'Confirm', 'Notify', 'Character', 'DynamicCharacter', 'NamedCharacter'):
            if node.args:
                val = get_ast_string(node.args[0])
                if val and not val.endswith('()'):  # Фильтруем динамические имена (функции)
                    self.strings.append(val)

        # ТОЧЕЧНЫЙ ФИКС №4: renpy.input("prompt") — промпт ввода (имя игрока и т.п.).
        # Узкий случай: API renpy.input, первый позиционный аргумент — всегда видимый
        # игроку промпт. Автор часто НЕ оборачивает его в _(), поэтому ни движок, ни мы
        # его иначе не видим. Доставка — через рантайм-обёртку renpy.input в патче.
        if (isinstance(node.func, python_ast.Attribute) and node.func.attr == 'input'
                and isinstance(node.func.value, python_ast.Name)
                and node.func.value.id == 'renpy'):
            if node.args:
                val = get_ast_string(node.args[0])
                if val and val.strip():
                    self.strings.append(val)
        self.generic_visit(node)

    def visit_Assign(self, node):
        """Ловим legacy displayDict присваивания: displayDict["en"].key = "value" """
        for target in node.targets:
            if isinstance(target, python_ast.Attribute):
                if isinstance(target.value, python_ast.Subscript):
                    subscript_val = target.value.value
                    if isinstance(subscript_val, python_ast.Name) and subscript_val.id == 'displayDict':
                        attr_name = target.attr
                        # Фильтруем технические ключи
                        skip_keys = {'font', 'sayfont', 'language', 'line_spacing', 'timeformat',
                                    'selector_padding', 'nvl_paragraph_distance', 'gm_spacing',
                                    'ui_line_spacing', 'styleoverrides'}
                        if attr_name in skip_keys:
                            break
                        # Извлекаем строковое значение
                        val = get_ast_string(node.value)
                        if val and val.strip() and len(val.strip()) > 1:
                            # Специальная обработка: name_XX → Character definition
                            if attr_name.startswith('name_'):
                                char_code = attr_name[5:]  # "name_hi" → "hi"
                                self.char_definitions.append((char_code, val))
                            else:
                                self.strings.append(val)
                        # Если значение — список строк
                        elif isinstance(node.value, (python_ast.List, python_ast.Tuple)):
                            for elt in node.value.elts:
                                elt_val = get_ast_string(elt)
                                if elt_val and elt_val.strip() and len(elt_val.strip()) > 1:
                                    self.strings.append(elt_val)
        self.generic_visit(node)

FALLBACK_TRANSLATION_REGEX = re.compile(r'(?:_|\b__|\bConfirm|\bNotify|\bCharacter|\bDynamicCharacter|\bNamedCharacter)\(\s*(["\'])(.*?)(?<!\\)\1\s*[,)]')

# Legacy Ren'Py: displayDict["lang"].key = "value" или u"value"
LEGACY_DISPLAYDICT_REGEX = re.compile(r'displayDict\[["\'][^"\']+["\']\]\.(\w+)\s*=\s*u?(["\'])((?:(?!\2).)*)\2')

# Legacy Ren'Py: ui.textbutton("text", ...) / ui.text("text") / ui.label("text")
LEGACY_UI_CALL_REGEX = re.compile(r'ui\.(?:textbutton|text|label|button)\s*\(\s*(?:_\(\s*)?u?(["\'])((?:(?!\1).)*)\1')

# renpy.input("prompt", ...) — промпт ввода (имя игрока и т.п.), часто без _()
RENPY_INPUT_REGEX = re.compile(r'renpy\.input\s*\(\s*(?:_\(\s*)?u?(["\'])((?:(?!\1).)*)\1')

# --- Мета об игре (config.name / config.version) ---
GAME_META = {}
_META_LITERAL = re.compile(r'["\']([^"\']+)["\']')

def capture_game_meta(varname, source):
    """Define-узел: varname — имя переменной, source — правая часть (RHS).
    Берём первый строковый литерал из RHS для известных имён."""
    if not varname:
        return
    m = _META_LITERAL.search(source or '')
    if not m:
        return
    val = m.group(1).strip()
    if not val:
        return
    if varname in ('config.name', 'gui.name'):
        GAME_META.setdefault('name', val)
    elif varname == 'name':
        GAME_META.setdefault('name_fb', val)
    elif varname in ('config.version', 'gui.version'):
        GAME_META.setdefault('version', val)
    elif varname == 'version':
        GAME_META.setdefault('version_fb', val)

def extract_python_strings(code_string):
    if not isinstance(code_string, str) or not code_string.strip():
        return [], []
    try:
        parsed = python_ast.parse(code_string)
        visitor = RenpyTranslationVisitor()
        visitor.visit(parsed)
        return visitor.strings, visitor.char_definitions
    except SyntaxError:
        # Фоллбэк для Python 2 кода и legacy Ren'Py
        results = []
        char_defs = []
        
        # Стандартные _(), Character() и т.д.
        matches = FALLBACK_TRANSLATION_REGEX.findall(code_string)
        results.extend(match[1] for match in matches)
        
        # Legacy displayDict присваивания
        for match in LEGACY_DISPLAYDICT_REGEX.finditer(code_string):
            key = match.group(1)
            value = match.group(3)
            skip_keys = {'font', 'sayfont', 'language', 'line_spacing', 'timeformat',
                        'selector_padding', 'nvl_paragraph_distance', 'gm_spacing', 
                        'ui_line_spacing', 'styleoverrides'}
            if key not in skip_keys and value.strip():
                if key.startswith('name_'):
                    char_code = key[5:]
                    char_defs.append((char_code, value))
                else:
                    results.append(value)
        
        # Legacy ui.textbutton/ui.text вызовы
        for match in LEGACY_UI_CALL_REGEX.finditer(code_string):
            value = match.group(2)
            if value.strip():
                results.append(value)

        # renpy.input(...) промпты
        for match in RENPY_INPUT_REGEX.finditer(code_string):
            value = match.group(2)
            if value.strip():
                results.append(value)

        return results, char_defs

def extract_implicit_string(expr):
    if not isinstance(expr, str) or not expr.strip():
        return None
    try:
        parsed = python_ast.parse(expr.strip(), mode='eval')
        return get_ast_string(parsed.body)
    except SyntaxError:
        return None

# ТОЧЕЧНЫЙ ФИКС №3: Умный парсер для кастомных экранов (use animbutton ("Start", ...))
def extract_targeted_use_args(expr):
    if not isinstance(expr, str) or not expr.strip(): return []
    strings = []
    try:
        # Оборачиваем аргументы в вызов функции, чтобы AST распарсил их как список аргументов
        parsed = python_ast.parse(f"dummy_func({expr})")
        if parsed.body and isinstance(parsed.body[0], python_ast.Expr):
            call_node = parsed.body[0].value
            if isinstance(call_node, python_ast.Call):
                # Извлекаем ТОЛЬКО прямые позиционные строки (игнорируя вложенные вызовы типа ShowMenu)
                for arg_node in call_node.args:
                    val = get_ast_string(arg_node)
                    # Базовая защита от извлечения системных ID (типа gallery_cg)
                    if val and not re.match(r'^[a-z0-9_]+$', val):
                        strings.append(val)
    except SyntaxError:
        pass
    return strings

# =====================================================================

def _sl1_str_value(node):
    """Строковое значение из mock _ast-узла (Str/Constant/Num), иначе None."""
    cn = getattr(node, '__class_name__', None)
    if cn == 'Str':
        v = getattr(node, 's', None)
    elif cn in ('Constant', 'Num'):
        v = getattr(node, 'value', None)
    else:
        return None
    return v if isinstance(v, (str, bytes)) else None

def extract_sl1_ui_strings(code_obj):
    """SL1 (Ren'Py 6.12–6.17): скрин компилируется в Python (ui.text/textbutton/label).
    AST лежит в PyCode.code_ast (mock _ast). Берём ПЕРВЫЙ строковый аргумент этих вызовов —
    это и есть отображаемый UI-текст (а не имена стилей/картинок/прочий мусор)."""
    code_ast = getattr(code_obj, 'code_ast', None)
    if code_ast is None:
        return []
    out = []
    visited = set()
    def w(node, depth=0):
        if id(node) in visited or depth > 300:
            return
        visited.add(id(node))
        if getattr(node, '__class_name__', None) == 'Call':
            func = getattr(node, 'func', None)
            if getattr(func, 'attr', None) in ('text', 'textbutton', 'label'):
                val = getattr(func, 'value', None)
                if getattr(val, 'id', None) == 'ui':
                    args = getattr(node, 'args', None)
                    if isinstance(args, (list, tuple)) and args:
                        sv = _sl1_str_value(args[0])
                        if sv is not None:
                            out.append(sv)
        if isinstance(node, (list, tuple)):
            for x in node:
                w(x, depth + 1)
        elif isinstance(node, dict):
            for x in node.values():
                w(x, depth + 1)
        elif hasattr(node, '__dict__'):
            for x in node.__dict__.values():
                w(x, depth + 1)
    w(code_ast)
    return out


def extract_layeredimage_strings(block, results, _depth=0):
    """layeredimage — это UserStatement; его .block хранит исходные строки подэлементов
    (group/attribute/always/...) рекурсивными кортежами (file, line, "source", children).
    Внутри встречаются displayable-выражения вида Text(_("YES"), style=...). Движок переводит
    такие _() при сборке картинки, а наш AST-обход их раньше не видел (текст в .parsed-узлах
    RawAttribute.image, не в обычных Say/Screen). Извлекаем _() прямо из исходника блока —
    extract_python_strings берёт только _()/__()/Character, поэтому пути/имена стилей не шумят."""
    if _depth > 40 or not isinstance(block, (list, tuple)):
        return
    for item in block:
        if isinstance(item, (list, tuple)) and len(item) >= 4 and isinstance(item[2], str):
            raw_file = item[0] if isinstance(item[0], str) else 'unknown'
            ln = item[1] if isinstance(item[1], int) else 0
            found, _cd = extract_python_strings(item[2])
            for text in found:
                safe = sanitize_string(text)
                if safe and safe.strip():
                    results.append({
                        "type": "ui",
                        "id": safe,
                        "file": clean_filename(raw_file),
                        "line": ln,
                        "who": "[ИНТЕРФЕЙС]",
                        "what": safe,
                    })
            extract_layeredimage_strings(item[3], results, _depth + 1)
        elif isinstance(item, (list, tuple)):
            extract_layeredimage_strings(item, results, _depth + 1)


def explore_ast(node, results, depth=0, visited=None, current_label="", pending_prefix=None, translate_id=None):
    if visited is None:
        visited = set()
        
    if isinstance(node, (str, int, float, bool, bytes)) or node is None:
        return
        
    node_id = id(node)
    if node_id in visited:
        return
    visited.add(node_id)

    if isinstance(node, (tuple, list)):
        current_prefix = pending_prefix
        for item in node:
            item_class = getattr(item, '__class_name__', '')
            
            if item_class == 'UserStatement':
                line = getattr(item, 'line', '')
                if isinstance(line, str) and line.startswith('voice '):
                    current_prefix = line.strip()
                    
            explore_ast(item, results, depth, visited, current_label, current_prefix, translate_id)
            
            if item_class in ('Say', 'Menu', 'Translate'):
                current_prefix = None
            elif item_class not in ('UserStatement', 'Python', 'EarlyPython', 'Pass'):
                current_prefix = None
                
    elif isinstance(node, dict):
        for key, value in node.items(): 
            explore_ast(value, results, depth, visited, current_label, pending_prefix, translate_id)
            
    elif hasattr(node, '__class_name__') or hasattr(node, '__module__'):
        class_name = getattr(node, '__class_name__', '')
        module_name = getattr(node, '__module__', '')
        
        if class_name == 'Label':
            current_label = getattr(node, 'name', current_label)
            
        if class_name == 'Translate':
            t_id = getattr(node, 'identifier', '')
            if hasattr(node, 'block'):
                explore_ast(node.block, results, depth + 1, visited, current_label, pending_prefix, translate_id=t_id)
            return

        # --- layeredimage: _() в displayable-выражениях (Text(_("...")) и т.п.) ---
        # Текст вкладок/кнопок кастомных меню часто задаётся как Text(_("...")) внутри
        # layeredimage (напр. Refuge of Embers: вкладки меню, YES/NO, названия языков).
        if class_name == 'UserStatement':
            _us_line = getattr(node, 'line', '')
            if isinstance(_us_line, str) and _us_line.strip().startswith('layeredimage'):
                extract_layeredimage_strings(getattr(node, 'block', None), results)
        
        # --- 1. ДИАЛОГИ (Say) ---
        if class_name == 'Say':
            who = sanitize_string(getattr(node, 'who', None))
            what = sanitize_string(getattr(node, 'what', None))
            
            if what: 
                location = getattr(node, 'location', None)
                if location:
                    raw_file, line = location[0], location[1]
                else:
                    raw_file = getattr(node, 'filename', 'unknown')
                    line = getattr(node, 'linenumber', getattr(node, 'line', 0))
                
                filename = clean_filename(raw_file)
                
                say_id = getattr(node, 'translation_identifier', getattr(node, 'identifier', getattr(node, 'id', None)))
                if not say_id and translate_id:
                    say_id = translate_id
                    
                if not say_id:
                    say_id = generate_real_renpy_id(current_label, filename, node, pending_prefix)
                else:
                    say_id = sanitize_string(say_id)
                
                results.append({
                    "type": "dialogue",
                    "id": say_id,
                    "file": filename,
                    "line": line,
                    "who": who,
                    "what": what,
                    "prefix": pending_prefix 
                })

        # --- 2. ВЫБОРЫ (Menu) ---
        elif class_name == 'Menu':
            items = getattr(node, 'items', [])
            for item in items:
                if isinstance(item, tuple) and len(item) >= 3:
                    label = sanitize_string(item[0])
                    block = item[2]  
                    
                    if isinstance(label, str) and label:
                        location = getattr(node, 'location', None)
                        if location:
                            raw_file, line = location[0], location[1]
                        else:
                            raw_file = getattr(node, 'filename', 'unknown')
                            line = getattr(node, 'linenumber', getattr(node, 'line', 0))
                        
                        filename = clean_filename(raw_file)
                        
                        results.append({
                            "type": "menu",
                            "id": label,
                            "file": filename,
                            "line": line,
                            "who": "[ВЫБОР]",
                            "what": label
                        })
                        
                    explore_ast(block, results, depth + 1, visited, current_label)
            return

        # --- 3. ЭКРАНЫ (Screen) ---
        elif class_name == 'Screen':
            screen_node = getattr(node, 'screen_node', None)
            if screen_node:
                explore_ast(screen_node, results, depth + 1, visited, current_label)
            else:
                # SL1 (6.12–6.17): нет screen_node. Содержимое в .screen (ScreenLangScreen)
                # → .code (PyCode) → code_ast. Достаём UI-текст из ui.text/textbutton/label.
                sl_scr = getattr(node, 'screen', None)
                code_obj = getattr(sl_scr, 'code', None) if sl_scr is not None else None
                if code_obj is not None:
                    raw_file = getattr(node, 'filename', 'unknown')
                    line = getattr(node, 'linenumber', getattr(node, 'line', 0))
                    filename = clean_filename(raw_file)
                    for text in extract_sl1_ui_strings(code_obj):
                        safe_text = sanitize_string(text)
                        if safe_text and safe_text.strip():
                            results.append({
                                "type": "ui",
                                "id": safe_text,
                                "file": filename,
                                "line": line,
                                "who": "[ИНТЕРФЕЙС]",
                                "what": safe_text
                            })

        # --- 3.5. SHOW TEXT: show text "..." (текст как displayable в Show/Scene) ---
        # imspec = (name_tuple, expr, tag, at_list, layer, zorder, behind).
        # Для текста name_tuple == ('text', '"...литерал..."'); для картинок ('cg01','a01').
        elif class_name in ('Show', 'ShowImage', 'Scene'):
            imspec = getattr(node, 'imspec', None)
            if isinstance(imspec, (tuple, list)) and len(imspec) >= 1:
                name_tuple = imspec[0]
                if (isinstance(name_tuple, (tuple, list)) and len(name_tuple) >= 2
                        and name_tuple[0] == 'text'):
                    location = getattr(node, 'location', None)
                    if location:
                        raw_file, line = location[0], location[1]
                    else:
                        raw_file = getattr(node, 'filename', 'unknown')
                        line = getattr(node, 'linenumber', getattr(node, 'line', 0))
                    filename = clean_filename(raw_file)
                    for arg in name_tuple[1:]:
                        if not isinstance(arg, str):
                            continue
                        texts = []
                        found_explicit, _cd = extract_python_strings(arg)
                        if found_explicit:
                            texts.extend(found_explicit)
                        else:
                            impl = extract_implicit_string(arg)
                            if impl:
                                texts.append(impl)
                        for text in texts:
                            safe_text = sanitize_string(text)
                            if safe_text and safe_text.strip():
                                results.append({
                                    "type": "ui",
                                    "id": safe_text,
                                    "file": filename,
                                    "line": line,
                                    "who": "[SHOW TEXT]",
                                    "what": safe_text
                                })

        # --- 4. ЭЛЕМЕНТЫ UI ---
        # ДОБАВЛЕН класс SLUse в проверку для парсинга аргументов кастомных экранов
        elif ('sl2.slast' in module_name or 'screenlang' in module_name) or hasattr(node, 'positional') or hasattr(node, 'keyword') or class_name == 'SLUse':
            location = getattr(node, 'location', None)
            if location:
                raw_file, line = location[0], location[1]
            else:
                raw_file = getattr(node, 'filename', 'unknown')
                line = getattr(node, 'linenumber', getattr(node, 'line', 0))
            filename = clean_filename(raw_file)

            positional = getattr(node, 'positional', [])
            keyword = getattr(node, 'keyword', [])

            if isinstance(keyword, dict):
                keyword_items = list(keyword.items())
            else:
                keyword_items = keyword if keyword else []

            # Собираем все выражения для анализа (positional + keyword values + expression)
            expressions = []
            if isinstance(positional, list):
                expressions.extend(positional)
            
            if isinstance(keyword_items, (list, tuple)):
                for k_item in keyword_items:
                    if isinstance(k_item, tuple) and len(k_item) == 2:
                        key, val = k_item
                        if isinstance(val, str):
                            expressions.append(val)

            # Также проверяем атрибут 'expression' (SLDefault, SLIf и т.д.)
            expr_attr = getattr(node, 'expression', None)
            if expr_attr is not None and isinstance(expr_attr, str):
                expressions.append(expr_attr)

            for expr in expressions:
                # Получаем строковое значение: сначала str(expr), если пусто — .source
                expr_str = str(expr) if isinstance(expr, str) and len(expr) > 0 else ''
                if not expr_str and hasattr(expr, 'source') and isinstance(getattr(expr, 'source', None), str):
                    expr_str = expr.source
                
                if isinstance(expr_str, str) and expr_str:
                    found_explicit, _ = extract_python_strings(expr_str)
                    for text in found_explicit:
                        safe_text = sanitize_string(text)
                        results.append({
                            "type": "ui",
                            "id": safe_text,
                            "file": filename,
                            "line": line,
                            "who": "[ИНТЕРФЕЙС]",
                            "what": safe_text
                        })

            name = getattr(node, 'name', '')
            # ТОЧЕЧНЫЙ ФИКС №5: на старых движках (6.99) у SLDisplayable нет .name —
            # тип элемента лежит в .style ('text'/'textbutton'/'label'/'button').
            # Поэтому детектим текст-элемент и по name, и по style.
            sl_style = getattr(node, 'style', '')
            if not isinstance(sl_style, str):
                sl_style = ''
            _text_tags = ('text', 'textbutton', 'label', 'button')
            is_text_element = False
            prefix_who = ""

            if class_name == 'SLDisplayable' and (name in _text_tags or sl_style in _text_tags):
                is_text_element = True
                _tag = name if name in _text_tags else sl_style
                prefix_who = f"[{_tag.upper()}]"
            elif class_name == 'TextNode': 
                is_text_element = True
                prefix_who = "[TEXT]"
                
            if is_text_element and positional:
                # Получаем первый positional с учётом .source для пустых PyExpr
                first_pos = positional[0] if positional else None
                if first_pos is not None and isinstance(first_pos, str):
                    pos_str = str(first_pos) if len(first_pos) > 0 else ''
                    if not pos_str and hasattr(first_pos, 'source'):
                        pos_str = getattr(first_pos, 'source', '')
                    if pos_str:
                        implicit_text = extract_implicit_string(pos_str)
                        if implicit_text:
                            safe_impl = sanitize_string(implicit_text)
                            results.append({
                                "type": "ui",
                                "id": safe_impl,
                                "file": filename,
                                "line": line,
                                "who": prefix_who,
                                "what": safe_impl
                            })
            
            if isinstance(keyword_items, (list, tuple)):
                for k_item in keyword_items:
                    if isinstance(k_item, tuple) and len(k_item) == 2:
                        key, val = k_item
                        if key in ('tooltip', 'text_tooltip', 'prompt') and isinstance(val, str):
                            val_str = str(val) if len(val) > 0 else ''
                            if not val_str and hasattr(val, 'source'):
                                val_str = getattr(val, 'source', '')
                            if val_str:
                                implicit_text = extract_implicit_string(val_str)
                                if implicit_text:
                                    safe_impl = sanitize_string(implicit_text)
                                    results.append({
                                        "type": "ui",
                                        "id": safe_impl,
                                        "file": filename,
                                        "line": line,
                                        "who": f"[{key.upper()}]",
                                        "what": safe_impl
                                    })

            # ИЗВЛЕЧЕНИЕ АРГУМЕНТОВ КАСТОМНЫХ ЭКРАНОВ
            # use screen(_("Preferences")) / use file_slots(_("Load")) / animbutton("Start", ...)
            # SLUse.args — это ArgumentInfo с .source=None; реальные аргументы в .arguments
            # как список (имя, выражение): [(None, '_("Load")'), ('is_load', 'True')].
            if class_name == 'SLUse':
                use_args = getattr(node, 'args', None)
                target_screen = getattr(node, 'target', 'SCREEN')

                arg_items = getattr(use_args, 'arguments', None) if use_args is not None else None
                # Фоллбэк на старый путь (PyExpr-строка с .source), если .arguments нет
                if not isinstance(arg_items, (list, tuple)):
                    args_str = ''
                    if isinstance(use_args, str) and len(use_args) > 0:
                        args_str = str(use_args)
                    elif hasattr(use_args, 'source') and isinstance(getattr(use_args, 'source', None), str):
                        args_str = getattr(use_args, 'source')
                    arg_items = [(None, args_str)] if args_str else []

                for item in arg_items:
                    if not (isinstance(item, tuple) and len(item) == 2):
                        continue
                    arg_name, expr = item
                    # Берём только позиционные аргументы (имя=None); keyword пропускаем
                    if arg_name is not None:
                        continue
                    if not isinstance(expr, str) or not expr.strip():
                        continue

                    texts = []
                    # 1) Явные переводимые: _("..."), __("..."), Character("...") и т.п.
                    found_explicit, _cd = extract_python_strings(expr)
                    if found_explicit:
                        texts.extend(found_explicit)
                    else:
                        # 2) Голый строковый литерал: animbutton("Start", ...).
                        #    Защита от системных ID (gallery_cg): отсекаем чистые идентификаторы.
                        impl = extract_implicit_string(expr)
                        if impl and not re.match(r'^[a-z0-9_]+$', impl):
                            texts.append(impl)

                    for text in texts:
                        safe_text = sanitize_string(text)
                        if safe_text and safe_text.strip():
                            results.append({
                                "type": "ui",
                                "id": safe_text,
                                "file": filename,
                                "line": line,
                                "who": f"[USE: {target_screen}]",
                                "what": safe_text
                            })

        # --- 5. PYTHON СТРОКИ ---
        # ДОБАВЛЕНЫ узлы Define и Default, чтобы ловить определения персонажей
        elif class_name in ('Python', 'EarlyPython', 'Define', 'Default') or hasattr(node, 'code'):
            code_obj = getattr(node, 'code', None)
            if code_obj is not None:
                # PyExpr может хранить source тремя способами:
                # 1. Как строковое значение самого объекта (str(code_obj))
                # 2. В атрибуте .source (установленном через __setstate__)
                # 3. В атрибуте code_obj.source (если это объект с атрибутом)
                source = ''
                if isinstance(code_obj, str) and len(code_obj) > 0:
                    source = str(code_obj)
                elif hasattr(code_obj, 'source') and isinstance(getattr(code_obj, 'source', None), str):
                    source = getattr(code_obj, 'source')
                
                if source:
                    _vn = getattr(node, 'varname', None) if class_name in ('Define', 'Default') else None
                    capture_game_meta(_vn, source)
                    found_in_python, found_char_defs = extract_python_strings(source)
                    
                    if found_in_python:
                        location = getattr(node, 'location', None)
                        if location:
                            raw_file, line = location[0], location[1]
                        else:
                            raw_file = getattr(node, 'filename', 'unknown')
                            line = getattr(node, 'linenumber', getattr(node, 'line', 0))
                        filename = clean_filename(raw_file)

                        # Определение типа переменной для тега 'who'
                        prefix_who = f"[{class_name.upper()}]" if class_name else "[PYTHON]"
                        if class_name in ('Define', 'Default'):
                            var_name = getattr(node, 'varname', 'VAR')
                            prefix_who = f"[DEFINE: {var_name}]"

                        for text in found_in_python:
                            safe_text = sanitize_string(text)
                            results.append({
                                "type": "python",
                                "id": safe_text,
                                "file": filename,
                                "line": line,
                                "who": prefix_who,
                                "what": safe_text
                            })
                    
                    # Character definitions из displayDict.name_XX
                    if found_char_defs:
                        location = getattr(node, 'location', None)
                        if location:
                            raw_file, line = location[0], location[1]
                        else:
                            raw_file = getattr(node, 'filename', 'unknown')
                            line = getattr(node, 'linenumber', getattr(node, 'line', 0))
                        filename = clean_filename(raw_file)
                        
                        for char_code, char_name in found_char_defs:
                            safe_name = sanitize_string(char_name)
                            results.append({
                                "type": "python",
                                "id": safe_name,
                                "file": filename,
                                "line": line,
                                "who": f"[DEFINE: {char_code}]",
                                "what": safe_name
                            })

        # --- 6. ДОЧЕРНИЕ УЗЛЫ (Явные) ---
        if hasattr(node, 'block'): 
            explore_ast(node.block, results, depth + 1, visited, current_label, pending_prefix, translate_id)
        if hasattr(node, 'items'):
            for item in getattr(node, 'items', []): 
                explore_ast(item, results, depth + 1, visited, current_label, pending_prefix, translate_id)
        if hasattr(node, 'children'):
            for item in getattr(node, 'children', []): 
                explore_ast(item, results, depth + 1, visited, current_label, pending_prefix, translate_id)
        if hasattr(node, '_appended_items'):
            for item in getattr(node, '_appended_items', []):
                explore_ast(item, results, depth + 1, visited, current_label, pending_prefix, translate_id)

        # --- 7. ТОТАЛЬНЫЙ ОБХОД (Страховка) ---
        if hasattr(node, '__dict__'):
            for dict_key, dict_val in node.__dict__.items():
                if dict_key not in ('__module__', '__class_name__', 'block', 'items', 'children', '_appended_items', 'next'):
                    explore_ast(dict_val, results, depth + 1, visited, current_label, pending_prefix, translate_id)