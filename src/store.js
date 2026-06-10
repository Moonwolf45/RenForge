import { ref, watch } from 'vue';

// Настройки приложения
export const uiLang = ref(localStorage.getItem('renforge_ui_lang') || 'en');
export const targetLang = ref(localStorage.getItem('renforge_target_lang') || '');
// Целевая письменность для подсветки/подбора шрифтов. 'auto' = выводить из языка перевода.
export const targetScript = ref(localStorage.getItem('renforge_target_script') || 'auto');
export const sourceLang = ref(localStorage.getItem('renforge_source_lang') || '');
export const uiTheme = ref(localStorage.getItem('renforge_ui_theme') || 'dark');
// Акцентный цвет интерфейса (применяется поверх темы).
// Палитра намеренно приглушённая (пастельная): сниженная насыщенность мягче для глаз
// на AMOLED-теме (на чистом чёрном яркие насыщенные цвета «выжигают»/бликуют),
// при этом светлота держится так, чтобы белый текст на кнопках оставался читаемым.
export const ACCENTS = {
  blue:   { c: '#5b82c9', h: '#4a6fb3' },
  violet: { c: '#8f74c6', h: '#7d62b4' },
  teal:   { c: '#2f9183', h: '#277f72' },
  rose:   { c: '#cf6a83', h: '#bd566f' },
  orange: { c: '#c4743f', h: '#b06436' },
  green:  { c: '#52a06b', h: '#448d5c' },
};
export const uiAccent = ref(localStorage.getItem('renforge_ui_accent') || 'blue');

// Затемнение hex-цвета для производного hover-варианта (кастомный акцент).
export function darkenHex(hex, f = 0.84) {
  const m = /^#?([0-9a-fA-F]{6})$/.exec((hex || '').trim());
  if (!m) return hex;
  const n = parseInt(m[1], 16);
  let r = Math.round(((n >> 16) & 255) * f);
  let g = Math.round(((n >> 8) & 255) * f);
  let b = Math.round((n & 255) * f);
  return '#' + ((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1);
}

// uiAccent хранит ЛИБО ключ пресета ('blue'…), ЛИБО произвольный hex ('#rrggbb').
// resolveAccent понимает оба формата и возвращает { c, h } (цвет + hover).
export function resolveAccent(val) {
  const s = (val || '').trim();
  if (/^#?[0-9a-fA-F]{6}$/.test(s)) {
    const c = s.startsWith('#') ? s : '#' + s;
    return { c, h: darkenHex(c) };
  }
  return ACCENTS[val] || ACCENTS.blue;
}

// Контрастный цвет текста (чёрный/белый) для надписей на ЗАЛИВКЕ акцентом (кнопки,
// бейджи). Нужен, т.к. кастомный акцент может быть светлым — белый текст «слепнет».
// По относительной яркости sRGB (WCAG).
export function contrastFor(hex) {
  const m = /^#?([0-9a-fA-F]{6})$/.exec((hex || '').trim());
  if (!m) return '#ffffff';
  const n = parseInt(m[1], 16);
  const lin = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
  const L = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  return L > 0.55 ? '#1a1a1a' : '#ffffff';
}

// Шуточные системные промпты для пасхалки (троллинг). Формат нумерованного
// списка + плейсхолдеры сохранены, чтобы разбор ответа не ломался — меняется
// только «персона» модели. Сбрасывается кнопкой «Сбросить к стандарту».
export const FUNNY_PROMPTS = [
  `You are an EXTREMELY DRUNK visual novel translator who has had way too much to drink. Translate each line into {target_lang}, but slur your words, hiccup (*hic*), and make tipsy typos. Despite being absolutely hammered, you MUST still output exactly {count} lines as a numbered list (1 to {count}) and keep every tag like [name], {b}, {i}, \\n EXACTLY intact. {glossary}`,
  `Translate each line into {target_lang}, then reverse each translated sentence so it reads completely backwards (character by character). Output exactly {count} numbered lines (1 to {count}). Do NOT reverse the tags ([name], {b}, {i}, \\n) — keep them exactly as-is. {glossary}`,
  `You are a pompous Shakespearean playwright. Translate each line into {target_lang} using absurdly grandiose, archaic, over-the-top theatrical language ("hark!", "thou", "forsooth"). Output exactly {count} numbered lines (1 to {count}). Keep all tags [name], {b}, {i}, \\n intact. {glossary}`,
  `You are a salty pirate translator, arrr. Translate each line into {target_lang} but pepper every sentence with pirate slang (arrr, matey, ye scurvy dog, shiver me timbers). Output exactly {count} numbered lines (1 to {count}). Keep all tags [name], {b}, {i}, \\n intact. {glossary}`,
  `You are an ANGRY translator who HATES being asked to work. Translate each line into {target_lang} IN ALL CAPS while grumbling and complaining about your job. Output exactly {count} numbered lines (1 to {count}). Keep all tags [name], {b}, {i}, \\n intact. {glossary}`,
];
export const currentMode = ref('dashboard');
export const activePopover = ref(null);
export const showFontPanel = ref(false);
export const showUpdateModal = ref(false);
export const isAiModalOpen = ref(false);
export const showTmModal = ref(false);
export const showSourceModal = ref(false);
export const showAddStringModal = ref(false);
export const showDeliveryHooksModal = ref(false);
export const showAboutModal = ref(false);
// Блок ручной строки, открытый на редактирование (null = режим добавления).
export const manualEditTarget = ref(null);
export const availableLanguages = ref([]);

// Псевдо-файл для ручных строк (то, что юзер увидел в игре, но экстрактор не достал).
// Стабильный языконезависимый ключ; в UI показывается как t('manual_strings_file').
export const MANUAL_FILE = '__renforge_manual__';
// Рабочие пространства перевода (пары языков source->target) текущего проекта
export const translationPairs = ref([]);
export const activePair = ref('');

// Уведомления (тосты): стек сообщений с автоскрытием. Липкий тост (timeout=0) —
// для долгих операций (распаковка/извлечение/экспорт): обновляется на месте, не копится.
export const toasts = ref([]);
let toastSeq = 0;
let stickyId = null;
const toastTimers = new Map();

function clearStickyToast() {
  if (stickyId != null) { removeToast(stickyId); stickyId = null; }
}

export function showMsg(type, text, timeout = 8000) {
  const t = (text == null ? '' : String(text)).trim();
  // Пустой текст — сигнал снять текущий липкий тост (исп. напр. в отмене экспорта).
  if (!t) { clearStickyToast(); return; }

  if (timeout === 0) {
    // Липкий тост операции: обновляем существующий или создаём новый.
    if (stickyId != null) {
      const ex = toasts.value.find(x => x.id === stickyId);
      if (ex) { ex.type = type; ex.text = t; return; }
    }
    const id = ++toastSeq;
    stickyId = id;
    toasts.value.push({ id, type, text: t, sticky: true });
    return;
  }

  // Таймерный тост = финал операции: снимаем липкий, добавляем и автоскрываем.
  clearStickyToast();
  const id = ++toastSeq;
  toasts.value.push({ id, type, text: t, sticky: false });
  toastTimers.set(id, setTimeout(() => removeToast(id), timeout));
}

export function removeToast(id) {
  const i = toasts.value.findIndex(x => x.id === id);
  if (i >= 0) toasts.value.splice(i, 1);
  if (toastTimers.has(id)) { clearTimeout(toastTimers.get(id)); toastTimers.delete(id); }
  if (stickyId === id) stickyId = null;
}

// Закрыть все тосты (× / общий сброс).
export function closeMsg() {
  toastTimers.forEach((tm) => clearTimeout(tm));
  toastTimers.clear();
  toasts.value = [];
  stickyId = null;
}

// Данные проекта
export const projectPath = ref('');
export const isProcessing = ref(false);
// Идёт экспорт (для показа кнопки «Отмена экспорта» в уведомлении).
export const isExporting = ref(false);
export const projectFiles = ref({ rpa_files:[], rpyc_files:[], rpy_files:[], tl_files:[] });
export const fileStats = ref({});
export const charMap = ref({});

// Редактор переводов
export const parsedBlocks = ref([]);
export const currentFilePath = ref('');
export const rawFileText = ref('');
export const isEditorLoading = ref(false);
export const hideTranslated = ref(false);
export const focusedBlockId = ref(null);
export const newTerm = ref({ original: '', translation: '' });
// Несохранённые изменения в редакторе + время последнего сохранения
export const editorDirty = ref(false);
export const lastSavedAt = ref('');
// Счётчик-сигнал для редактора: бамп заставляет пересчитать высоту всех textarea
// (например, после пакетного перевода, который заполняет многострочные переводы).
export const editorResizeTick = ref(0);

// Локально сохраненные списки (в LocalStorage)
export const hiddenFiles = ref([]);
export const completedFiles = ref([]);
export const fileNotes = ref({});
export const glossary = ref([]);
export const hiddenImages = ref([]);
export const hiddenAudio = ref([]);
export const hiddenFolders = ref([]); 
export const showHidden = ref(false);
export const showHiddenMedia = ref(false);
export const searchQuery = ref('');
export const searchResults = ref([]);

// Утилиты
export function getProjectKey(baseKey) {
  if (!projectPath.value) return null;
  return `${baseKey}_${projectPath.value.replace(/[^a-zA-Z0-9]/g, '_')}`;
}

function safeParseJSON(key, defaultVal = '[]') {
  try { return JSON.parse(localStorage.getItem(key) || defaultVal); } 
  catch (e) { return JSON.parse(defaultVal); }
}

export function loadProjectSettings() {
  if (!projectPath.value) return;
  hiddenFiles.value = safeParseJSON(getProjectKey('renforge_hidden'));
  completedFiles.value = safeParseJSON(getProjectKey('renforge_completed'));
  fileNotes.value = safeParseJSON(getProjectKey('renforge_file_notes'), '{}');
  glossary.value = safeParseJSON(getProjectKey('renforge_glossary'));
  hiddenImages.value = safeParseJSON(getProjectKey('renforge_hidden_img'));
  hiddenAudio.value = safeParseJSON(getProjectKey('renforge_hidden_aud'));
  hiddenFolders.value = safeParseJSON(getProjectKey('renforge_hidden_folders'));
  // Языки источника/перевода — пер-проектные. Для свежего проекта остаются «не
  // указаны» (''), чтобы пользователь осознанно выбрал их перед извлечением.
  sourceLang.value = localStorage.getItem(getProjectKey('renforge_source_lang')) || '';
  targetLang.value = localStorage.getItem(getProjectKey('renforge_target_lang')) || '';
}

// Авто-сохранение списков
const watchConfig = { deep: true };
watch(hiddenFiles, (val) => { const k = getProjectKey('renforge_hidden'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
watch(completedFiles, (val) => { const k = getProjectKey('renforge_completed'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
watch(fileNotes, (val) => { const k = getProjectKey('renforge_file_notes'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
watch(glossary, (val) => { const k = getProjectKey('renforge_glossary'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
watch(hiddenImages, (val) => { const k = getProjectKey('renforge_hidden_img'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
watch(hiddenAudio, (val) => { const k = getProjectKey('renforge_hidden_aud'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
watch(hiddenFolders, (val) => { const k = getProjectKey('renforge_hidden_folders'); if(k) localStorage.setItem(k, JSON.stringify(val)); }, watchConfig);
// Пер-проектное сохранение языков (плюс глобальный дефолт для новых проектов)
watch(sourceLang, (val) => { const k = getProjectKey('renforge_source_lang'); if(k) localStorage.setItem(k, val); localStorage.setItem('renforge_source_lang', val); });
watch(targetLang, (val) => { const k = getProjectKey('renforge_target_lang'); if(k) localStorage.setItem(k, val); localStorage.setItem('renforge_target_lang', val); });

export function getFileName(fullPath) { 
  if (!fullPath) return '';
  return fullPath.split(/[/\\]/).pop(); 
}

export function getRelativePath(fullPath) {
  if (!projectPath.value) return fullPath;
  const normalizedFull = fullPath.replace(/\\/g, '/');
  const normalizedProj = projectPath.value.replace(/\\/g, '/');
  return normalizedFull.replace(normalizedProj, '').replace(/^\//, '');
}

export function getFolderFromPath(relPath) {
  let normalized = relPath.replace(/\\/g, '/');
  const parts = normalized.split('/');
  if (parts.length > 1) { parts.pop(); return parts.join('/'); }
  return '/';
}