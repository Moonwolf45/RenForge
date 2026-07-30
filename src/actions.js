import { invoke } from '@tauri-apps/api/core';
import { save, open, ask } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { 
  projectPath, targetLang, sourceLang, isProcessing, projectFiles, charMap, fileStats, 
  showMsg, currentMode, isEditorLoading, currentFilePath, 
  parsedBlocks, hideTranslated, availableLanguages,
  editorDirty, lastSavedAt, translationPairs, activePair, isExporting, MANUAL_FILE, dupMap, diagnosticBuild
} from './store.js';
import { t } from './locales.js';

export async function refreshProject() {
  try { 
      isProcessing.value = true; 
      projectFiles.value = await invoke('scan_project', { path: projectPath.value, targetLang: targetLang.value }); 
      charMap.value = await invoke('get_character_mapping', { projectPath: projectPath.value });
      fileStats.value = await invoke('get_translation_stats', { projectPath: projectPath.value });
      // Языки-источники определяем сразу при открытии — селектор «Переводить с»
      // доступен ДО извлечения (для loose-file игр; для .rpa — после распаковки).
      try {
          const langs = await invoke('discover_source_languages', { projectPath: projectPath.value });
          if (langs && langs.length > 0) availableLanguages.value = langs;
      } catch (e) { console.error('discover_source_languages failed:', e); }
      await loadPairs();
  } 
  catch (e) { showMsg('error', `Error: ${e}`); } 
  finally { isProcessing.value = false; }
}

// === Рабочие пространства перевода (пары языков) ===
export async function loadPairs() {
  if (!projectPath.value) return;
  try {
    const pairs = await invoke('list_translation_pairs', { projectPath: projectPath.value });
    translationPairs.value = pairs || [];
    const act = (pairs || []).find(p => p.is_active);
    activePair.value = act ? act.pair : '';
  } catch (e) { console.error('loadPairs failed:', e); }
}

export async function switchPair(p) {
  if (!projectPath.value || p.is_active) return;
  try {
    isProcessing.value = true;
    if (p.is_legacy) {
      await invoke('use_legacy_db', { projectPath: projectPath.value });
    } else {
      await invoke('set_active_pair', { projectPath: projectPath.value, source: p.source || 'original', target: p.target || 'russian' });
      // подтянем язык пары в настройки
      if (p.source) sourceLang.value = p.source;
      if (p.target) targetLang.value = p.target;
    }
    await refreshProject();
    await loadPairs();
    showMsg('success', `Активная пара: ${p.is_legacy ? 'legacy' : (p.source + ' → ' + p.target)}`, 4000);
  } catch (e) { showMsg('error', e.toString()); }
  finally { isProcessing.value = false; }
}

export async function deletePair(p) {
  if (!projectPath.value || p.is_legacy) return;
  const ok = await ask(`Удалить рабочее пространство ${p.source} → ${p.target}? Переводы этой пары будут потеряны.`,
    { title: 'Удалить пару', kind: 'warning' });
  if (!ok) return;
  try {
    await invoke('delete_translation_pair', { projectPath: projectPath.value, pair: p.pair });
    await loadPairs();
    if (p.is_active) await refreshProject();
  } catch (e) { showMsg('error', e.toString()); }
}

export async function prepareProject() {
    // Гейт «осознанного выбора»: не извлекаем, пока пользователь не выбрал ОБА языка.
    // Это страхует от багов с автодефолтами (контаминация корпуса, неверная пара).
    if (!sourceLang.value || !targetLang.value) {
        showMsg('error', t('msg_pick_langs'), 9000);
        return;
    }
    isProcessing.value = true;
    
    // Снимаем атрибут read-only со всего проекта ДО распаковки/извлечения —
    // иначе запись (.rpa-распаковка, декомпиляция, tl/, БД) молча теряет файлы.
    try { await invoke('prepare_writable', { projectPath: projectPath.value }); } catch (e) { console.error(e); }

    // Проверка прав на запись: игра в Program Files/Steam под UAC недоступна для записи.
    // Без этой проверки распаковка/патч/БД молча падали бы. Прерываем с понятным советом.
    try {
        const writable = await invoke('is_path_writable', { projectPath: projectPath.value });
        if (!writable) {
            showMsg('error', t('msg_not_writable'), 15000);
            isProcessing.value = false;
            return;
        }
    } catch (e) { console.error(e); }

    // Распаковка .rpa архивов (оставляем как было)
    const rpa = projectFiles.value.rpa_files;
    for (let i = 0; i < rpa.length; i++) {
        showMsg('success', `${t('unpacking')} ${i+1} / ${rpa.length}...`, 0);
        try { await invoke('run_unrpa', { filePath: rpa[i] }); } catch (e) { console.error(e); }
    }

    // .rpa могли распаковаться только что — обновим список языков-источников,
    // чтобы пользователь мог выбрать «Переводить с» (для .rpa-игр он до этого пуст).
    if (rpa.length > 0) {
        try {
            const langs = await invoke('discover_source_languages', { projectPath: projectPath.value });
            if (langs && langs.length > 0) availableLanguages.value = langs;
        } catch (e) { console.error('discover_source_languages failed:', e); }
    }

    // Защита от source == target (бессмысленный «перевод сам в себя»).
    const _src = (sourceLang.value || '').toLowerCase();
    const _tgt = (targetLang.value || '').toLowerCase();
    if (_src && _src !== 'auto' && _src !== 'original' && _src === _tgt) {
        const ok = await ask(
            `Язык-источник и язык перевода совпадают (${sourceLang.value}). Извлечение будет «переводом самого в себя». Продолжить?`,
            { title: 'Источник = Перевод', kind: 'warning' }
        );
        if (!ok) { isProcessing.value = false; return; }
    }

    // Извлечение: чистый AST-экстрактор (не запускает игру, не декомпилирует —
    // по полноте на паритете с движком, без вмешательства в игру).
    showMsg('success', t('msg_extracting'), 0);
    try {
        const result = await invoke('extract_and_ingest_project', {
            projectPath: projectPath.value,
            sourceLang: sourceLang.value,
            targetLang: targetLang.value
        });
        showMsg('success', t('msg_extracted').replace('{n}', result.total), 8000);

        // Roadmap 1.3: репорт файлов, пропущенных экстрактором (сбой разбора AST /
        // не удалось загрузить .rpyc). Не блокирует успех — отдельный тост-предупреждение
        // поверх, т.к. это частичная, а не фатальная проблема (остальные строки извлеклись).
        const skipped = result.skipped_files || [];
        if (skipped.length > 0) {
            const preview = skipped.slice(0, 5).join(', ') + (skipped.length > 5 ? ', …' : '');
            showMsg('warn', t('msg_extractor_skipped').replace('{n}', skipped.length).replace('{files}', preview), 15000);
        }

        // Загружаем доступные языки из БД
        try {
            const langs = await invoke('get_project_languages', { projectPath: projectPath.value });
            if (langs && langs.length > 0) {
                availableLanguages.value = langs;
            }
        } catch (e) { console.error('Failed to load languages:', e); }
        await loadPairs();
        
    } catch (e) {
        // Бэкенд возвращает коды ошибок извлечения — локализуем известные.
        const s = (e && e.toString) ? e.toString() : String(e);
        let msg;
        if (s.includes('game_dir_missing')) msg = t('msg_no_game_dir');
        else if (s.includes('extractor_spawn_failed')) msg = t('msg_extractor_spawn') + ' ' + s.replace(/^.*extractor_spawn_failed:?/, '').trim();
        else if (s.includes('extractor_error')) msg = t('msg_extractor_error') + '\n' + s.replace(/^.*extractor_error:?/, '').trim();
        else msg = s;
        showMsg('error', msg, 15000);
    }

    await refreshProject();
    checkLooseRpy();
    isProcessing.value = false;
}

// Стем пути без расширения .rpy/.rpyc, нормализованный (для сопоставления .rpy ↔ .rpyc).
function rpyStem(p) {
  return String(p).replace(/\\/g, '/').replace(/\.(rpyc|rpy)$/i, '').toLowerCase();
}

// Предупреждение: найдены «осиротевшие» .rpy (без парного .rpyc) — их экстрактор берёт
// грубым регекс-фоллбэком или пропускает (в смешанной игре). Совет: запустить игру один
// раз (движок скомпилирует .rpy→.rpyc), затем переизвлечь — тогда сработает точный AST.
function checkLooseRpy() {
  const pf = projectFiles.value || {};
  const rpycStems = new Set((pf.rpyc_files || []).map(rpyStem));
  const orphans = (pf.rpy_files || []).filter((p) => !rpycStems.has(rpyStem(p)));
  if (orphans.length > 0) {
    showMsg('warn', t('msg_loose_rpy').replace('{n}', orphans.length), 16000);
  }
}

// Локализованная сводка сборки из счётчиков бэкенда (BuildCounts):
// доставлено (диалоги/UI) / из них на проверку (перенос+память) / пропущено небезопасных.
function buildReportMsg(res) {
  if (!res) return '';
  let s = t('build_delivered').replace('{say}', res.say ?? 0).replace('{ui}', res.ui ?? 0);
  if (res.review > 0) s += ' ' + t('build_review').replace('{n}', res.review);
  if (res.skipped_bad > 0) s += ' ' + t('build_skipped_bad').replace('{n}', res.skipped_bad);
  return s;
}

export async function generateTranslations() {
  try {
    isProcessing.value = true; 
    showMsg('success', t('msg_engine_working'), 0);
    
    const res = await invoke('generate_translations', { 
      projectPath: projectPath.value, 
      targetLang: targetLang.value,
      diagnostic: diagnosticBuild.value
    });
    
    showMsg('success', buildReportMsg(res));
    await refreshProject();
  } catch (e) { 
    showMsg('error', e, 15000); 
  } finally { 
    isProcessing.value = false; 
  }
}

// Единая функция "Собрать мод" = генерация переводов + внедрение патча.
// fontRemaps — массив { source, target }, где source — rel_path шрифта игры,
// target — путь к целевому шрифту (null = встроенный DejaVuSans движка).
export async function buildMod(fontRemaps = []) {
  try {
    isProcessing.value = true;
    showMsg('success', t('msg_engine_working'), 0);
    
    // 1. Генерация файлов перевода
    const res = await invoke('generate_translations', { 
      projectPath: projectPath.value, 
      targetLang: targetLang.value,
      diagnostic: diagnosticBuild.value
    });
    
    // 2. Внедрение патча
    await invoke('apply_renforge_patch', { 
      projectPath: projectPath.value, 
      targetLang: targetLang.value, 
      fontRemaps: fontRemaps
    });
    
    showMsg('success', t('msg_patch_applied') + ' ' + buildReportMsg(res), 8000);
    await refreshProject();
  } catch (e) { 
    showMsg('error', `${e}`, 15000); 
  } finally { 
    isProcessing.value = false; 
  }
}

// Экспорт перевода для распространения.
// mode: 'full' — вся игра с впечённым модом («Простой путь»), 'mod' — только оверлей-файлы.
// pair — чип пары из виджета (берём из него target). Доступно только для собранной пары.
export async function exportTranslation(pair, mode) {
  if (!projectPath.value) return;
  const target = (pair && pair.target) || targetLang.value;
  if (!target) { showMsg('error', t('msg_pick_langs'), 8000); return; }

  // Предупреждение: мод собран, но БД менялась после сборки — экспортируется старая сборка.
  if (pair && pair.is_built && pair.is_dirty) {
    const ok = await ask(t('export_dirty_warn'), { title: t('export_translation'), kind: 'warning' });
    if (!ok) return;
  }

  let unlisten = null;
  try {
    const dir = await open({ directory: true, multiple: false, title: t('export_choose_dir') });
    if (!dir) return;
    const base = projectPath.value.split(/[/\\]/).filter(Boolean).pop() || 'game';
    const tag = mode === 'full' ? t('export_tag_full') : t('export_tag_mod');
    const safe = `${base} - RenForge ${target} ${tag}`.replace(/[<>:"/\\|?*]/g, '_');
    const outRoot = `${dir}/${safe}`;

    isProcessing.value = true;
    isExporting.value = true;
    showMsg('success', t('exporting'), 0);

    // Прогресс копирования (особенно важен для «Полной игры» — многогигабайтные копии).
    unlisten = await listen('export_progress', (e) => {
      const { done = 0, total = 0 } = e.payload || {};
      const pct = total ? Math.round((done / total) * 100) : 0;
      showMsg('success', `${t('exporting')} ${pct}% (${done}/${total})`, 0);
    });

    const runExport = (overwrite) => invoke('export_translation', {
      projectPath: projectPath.value, targetLang: target, mode, outRoot, overwrite,
    });

    let res = await runExport(false);
    // Папка уже существует — спросим и перезапишем (очистим устаревшие файлы).
    if (res && res.code === 'exists') {
      const ok = await ask(t('export_exists_confirm'), { title: t('export_translation'), kind: 'warning' });
      if (!ok) { showMsg('success', '', 1); return; }
      res = await runExport(true);
    }

    if (res && res.code === 'cancelled') {
      showMsg('success', t('export_cancelled'), 6000);
    } else if (!res || res.code === 'done') {
      let msg = mode === 'full'
        ? t('export_done_full').replace('{files}', res.files).replace('{mb}', res.mb.toFixed(1))
        : t('export_done_mod').replace('{files}', res.files);
      if (res.skipped > 0) msg += ' ' + t('export_skipped').replace('{n}', res.skipped);
      showMsg('success', msg, 12000);
      try { await invoke('open_in_explorer', { path: outRoot }); } catch (e) { /* не критично */ }
    } else if (res.code === 'nospace') {
      showMsg('error', t('export_nospace').replace('{need}', res.need_gb.toFixed(1)).replace('{avail}', res.avail_gb.toFixed(1)), 15000);
    }
  } catch (e) {
    showMsg('error', e.toString(), 12000);
  } finally {
    if (unlisten) unlisten();
    isProcessing.value = false;
    isExporting.value = false;
  }
}

// Отмена текущего экспорта (кнопка в уведомлении). Бэкенд остановится в ближайшей итерации.
export async function cancelExport() {
  try { await invoke('cancel_export'); } catch (e) { /* ignore */ }
}

// Заливка из Translation Memory: точные совпадения оригинала → перевод в непереведённые
// строки активной пары, помечаются «к проверке». Возвращает число заполненных.
export async function tmFill() {
  if (!projectPath.value) return;
  try {
    isProcessing.value = true;
    const n = await invoke('tm_fill', { projectPath: projectPath.value });
    if (n > 0) { await refreshProject(); }
    showMsg('success', `${t('tm_filled')} ${n}.`, 7000);
  } catch (e) { showMsg('error', e.toString(), 12000); }
  finally { isProcessing.value = false; }
}

// Откат мода: убирает внедрённую в игру доставку RenForge (патч, рантайм-перевод, шрифты,
// кэш). БД переводов и локализованные медиа в tl/<target> остаются.
export async function removeMod(pair) {
  if (!projectPath.value) return;
  const ok = await ask(t('remove_mod_confirm'), { title: t('remove_mod'), kind: 'warning' });
  if (!ok) return;
  try {
    isProcessing.value = true;
    await invoke('remove_renforge_mod', { projectPath: projectPath.value, targetLang: (pair && pair.target) || targetLang.value });
    showMsg('success', t('remove_mod_done'), 6000);
    await loadPairs();
  } catch (e) { showMsg('error', e.toString(), 12000); }
  finally { isProcessing.value = false; }
}

// Перенос перевода из прошлой версии игры (fuzzy-миграция).
export async function migrateTranslations(oldProjectPath) {
  const report = await invoke('migrate_translations', {
    newProjectPath: projectPath.value,
    oldProjectPath: oldProjectPath,
  });
  await refreshProject();
  return report;
}

// -- Редактор: Работа с базой данных --
// Диагностики строки вынесены в diagnostics.js (единый реестр + автофиксы). Здесь —
// тонкие ре-экспорты, чтобы существующие импорты/шаблон не меняли путь.
export { getOriginalTags, getMissingTags, getExtraInterps } from './diagnostics.js';
import { blockStatus } from './diagnostics.js';
export function getBlockStatus(block) { return blockStatus(block); }

// Карта дубликатов оригинала (для пометок дубль/конфликт в редакторе). Фоново, не блокирует
// открытие: обновляется при открытии файла и после сохранения. Ошибка не критична — пустая карта.
export async function loadDupMap() {
  try {
    dupMap.value = await invoke('get_duplicate_originals', { projectPath: projectPath.value });
  } catch (e) { dupMap.value = {}; }
}

export async function openEditor(dbFilePath, opts = {}) {
  try {
    isEditorLoading.value = true;
    currentMode.value = 'editor';
    hideTranslated.value = false;
    currentFilePath.value = dbFilePath; // Сохраняем имя файла (например "script.rpy")
    
    // Запрашиваем строки напрямую из SQLite
    const entries = await invoke('get_translations_for_file', { 
        projectPath: projectPath.value, 
        filePath: dbFilePath 
    });
    
    parsedBlocks.value = entries;
    loadDupMap(); // фоновая карта дубликатов → пометки строк в редакторе
    editorDirty.value = false;
    
    // opts.silent — открытие не-извлечённого файла намеренно (кнопка «Открыть исходник…»),
    // тогда «нет строк» это норма, а не повод для предупреждения.
    if (entries.length === 0 && dbFilePath !== MANUAL_FILE && !opts.silent) {
        showMsg('error', t('msg_no_strings_in_file'));
    }
  } catch (e) { 
    showMsg('error', `Error: ${e}`); 
    currentMode.value = 'dashboard'; 
  } 
  finally { isEditorLoading.value = false; }
}

// Простой строковый хеш (djb2) для генерации id ручных строк по оригиналу.
function hashStr(s) {
  let h = 5381;
  for (let i = 0; i < s.length; i++) h = ((h << 5) + h + s.charCodeAt(i)) >>> 0;
  return h.toString(16);
}

// Ручное добавление строки (закрывает пропуски экстракции). Доставка text-keyed:
// нужны только оригинал + перевод + канал (dialogue/menu -> say_map, ui -> ui_map).
// file_path и line_number для доставки КОСМЕТИЧЕСКИ (рантайм матчит по тексту) — служат
// лишь навигации в редакторе. По умолчанию строка идёт в псевдо-файл «Ручные строки»;
// при toCurrentFile=true кладётся в текущий открытый файл с якорем lineNumber (для контекста).
// type: 'dialogue' | 'ui'. Возвращает id блока (для прокрутки к нему).
export async function addManualString(original, translation, type, toCurrentFile, lineNumber) {
  const orig = original || '';
  if (!orig.trim()) return null;
  const blockType = type === 'dialogue' ? 'dialogue' : 'ui';
  const id = 'manual_' + hashStr(blockType + '\u0000' + orig);
  const tr = (translation || '').trim();
  const useCurrent = !!toCurrentFile && currentFilePath.value && currentFilePath.value !== MANUAL_FILE;
  const filePath = useCurrent ? currentFilePath.value : MANUAL_FILE;
  const ln = useCurrent ? (parseInt(lineNumber, 10) || 0) : 0;

  const block = {
    id,
    block_type: blockType,
    file_path: filePath,
    line_number: ln,
    who: blockType === 'ui' ? '[ИНТЕРФЕЙС]' : '',
    original: orig,
    translation: translation || '',
    status: tr ? 'translated' : 'untranslated',
    prefix: null,
    prev_original: null,
  };

  if (filePath === currentFilePath.value) {
    // Цель — текущий открытый файл (реальный или сам MANUAL_FILE): вставляем в список
    // на позицию по line_number (сохранится по кнопке «Сохранить»).
    const idx = parsedBlocks.value.findIndex(b => b.id === id);
    if (idx >= 0) {
      parsedBlocks.value[idx] = { ...parsedBlocks.value[idx], ...block };
    } else {
      let pos = parsedBlocks.value.findIndex(b => (b.line_number || 0) > ln);
      if (pos < 0) pos = parsedBlocks.value.length;
      parsedBlocks.value.splice(pos, 0, block);
    }
    editorDirty.value = true;
  } else {
    // Цель — «Ручные строки», а открыт другой файл: пишем сразу в БД.
    try {
      await invoke('upsert_translations_batch', { projectPath: projectPath.value, entries: [block] });
      fileStats.value = await invoke('get_translation_stats', { projectPath: projectPath.value });
      loadPairs();
    } catch (e) { showMsg('error', `Error: ${e}`); return null; }
  }
  return id;
}

// Является ли блок ручной строкой (id начинается с manual_).
export function isManualString(block) {
  return !!block && typeof block.id === 'string' && block.id.startsWith('manual_');
}

// Правка ручной строки на месте (тип/оригинал/перевод). id оставляем прежним — это
// просто ключ строки в БД (доставка матчит по тексту), смена original его не требует.
export function updateManualString(block, original, translation, type) {
  if (!block) return;
  const orig = original || '';
  if (!orig) return;
  const blockType = type === 'dialogue' ? 'dialogue' : 'ui';
  block.block_type = blockType;
  block.original = orig;
  block.translation = translation || '';
  block.who = blockType === 'ui' ? '[ИНТЕРФЕЙС]' : '';
  block.status = (translation || '').trim() ? 'translated' : 'untranslated';
  editorDirty.value = true;
}

// Удаление ручной строки: из БД (если уже сохранена) и из текущего списка редактора.
export async function deleteManualString(block) {
  if (!block) return;
  const ok = await ask(t('manual_delete_confirm'), { title: t('manual_delete'), kind: 'warning' });
  if (!ok) return;
  try {
    await invoke('delete_translations', { projectPath: projectPath.value, ids: [block.id] });
  } catch (e) { showMsg('error', `Error: ${e}`); return; }
  const idx = parsedBlocks.value.findIndex(b => b.id === block.id);
  if (idx >= 0) parsedBlocks.value.splice(idx, 1);
  try {
    fileStats.value = await invoke('get_translation_stats', { projectPath: projectPath.value });
  } catch (e) { /* не критично */ }
  loadPairs();
  showMsg('success', t('manual_deleted'));
}

export async function saveFile() {
  if (!currentFilePath.value) return;
  const hasErrors = parsedBlocks.value.some(block => getBlockStatus(block) === 'error');
  if (hasErrors) { showMsg('error', t('msg_cannot_save_errors')); return; }
  
  try {
    // Обновляем статусы перед отправкой
    parsedBlocks.value.forEach(b => b.status = getBlockStatus(b));
    
    await invoke('upsert_translations_batch', { projectPath: projectPath.value, entries: parsedBlocks.value });
    
    // Обновляем визуальный прогресс-бар в дашборде
    fileStats.value[currentFilePath.value] = { 
        total: parsedBlocks.value.length, 
        translated: parsedBlocks.value.filter(b => b.status === 'translated').length,
        outdated: parsedBlocks.value.filter(b => getBlockStatus(b) === 'outdated').length
    };
    showMsg('success', t('msg_file_saved'));
    editorDirty.value = false;
    lastSavedAt.value = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    loadDupMap(); // обновить пометки дубликатов/конфликтов после сохранения
    // Обновляем карточки пар: прогресс (translated/total) и статус «изменено» (built_dirty)
    // меняются в БД при upsert, но без перечитывания пар индикатор оставался бы старым.
    loadPairs();
    // Авто-наполнение Translation Memory переведёнными строками активной пары (фоном).
    invoke('tm_contribute', { projectPath: projectPath.value }).catch(() => {});
  } catch (e) { showMsg('error', `Error: ${e}`); }
}

// Пакетный экспорт строк активной пары во все форматы (CSV/JSON/PO), по файлу на исходник.
// Работает по активной БД пары — вызывающий (PairsWidget) переключает пару перед вызовом.
export async function exportAllStrings() {
  if (!projectPath.value) return;
  try {
    const dir = await open({ directory: true, multiple: false, title: t('export_choose_dir') });
    if (!dir) return;
    const base = projectPath.value.split(/[/\\]/).filter(Boolean).pop() || 'game';
    const safe = `${base} - RenForge strings`.replace(/[<>:"/\\|?*]/g, '_');
    const outRoot = `${dir}/${safe}`;

    isProcessing.value = true;
    showMsg('success', t('exporting'), 0);
    const res = await invoke('export_strings', {
      projectPath: projectPath.value,
      targetLang: targetLang.value || 'russian',
      outRoot,
    });
    showMsg('success', t('export_strings_done').replace('{files}', res.files).replace('{strings}', res.strings), 12000);
    try { await invoke('open_in_explorer', { path: outRoot }); } catch (e) { /* не критично */ }
  } catch (e) {
    showMsg('error', e.toString(), 12000);
  } finally {
    isProcessing.value = false;
  }
}

// Экспорт / Импорт (почти без изменений, адаптирован под новые объекты)
// Имя по умолчанию для одиночного экспорта = имя открытого файла + формат (script.rpy.po).
function defaultExportName(ext) {
    const base = (currentFilePath.value || 'export').split(/[/\\]/).filter(Boolean).pop() || 'export';
    return `${base}.${ext}`;
}

export async function exportCSV() {
    let csvContent = "ID;Original;Translation\n";
    parsedBlocks.value.forEach(b => {
        const orig = (b.original || "").replace(/"/g, '""').replace(/\n/g, "[BR]");
        let tran = (b.translation || "").replace(/"/g, '""').replace(/\n/g, "[BR]");
        if (/^[=+\-@]/.test(tran)) { tran = "'" + tran; }
        csvContent += `"${b.id}";"${orig}";"${tran}"\n`;
    });
    try {
        const savePath = await save({ defaultPath: defaultExportName('csv'), filters:[{ name: 'CSV', extensions:['csv'] }] });
        if (savePath) { await invoke('write_text_file', { path: savePath, content: csvContent }); showMsg('success', t('msg_csv_exported')); }
    } catch (e) { showMsg('error', `Error: ${e}`); }
}

export async function importCSV() {
    try {
        const selected = await open({ multiple: false, filters:[{ name: 'CSV', extensions: ['csv'] }] });
        if (!selected) return;
        const csvContent = await invoke('read_text_file', { path: selected });
        const lines = csvContent.split('\n');
        let updatedCount = 0;
        for (let i = 1; i < lines.length; i++) {
            const line = lines[i].trim();
            if (!line) continue;
            const match = line.match(/^"([^"]+)";"([^"]*)";"([^"]*)"$/);
            if (match) {
                const id = match[1];
                let tran = match[3].replace(/\[BR\]/g, "\n").replace(/""/g, '"');
                if (tran.startsWith("'") && /^[=+\-@]/.test(tran.substring(1))) tran = tran.substring(1);
                const block = parsedBlocks.value.find(b => b.id === id);
                if (block && tran) { block.translation = tran; updatedCount++; }
            }
        }
        if (updatedCount > 0) editorDirty.value = true;
        showMsg('success', `${t('msg_csv_imported')} ${updatedCount}.`);
    } catch (e) { showMsg('error', `Error: ${e}`); }
}

export async function exportJSON() {
    const data = parsedBlocks.value.map(b => ({ id: b.id, original: b.original, translation: b.translation }));
    try {
        const savePath = await save({ defaultPath: defaultExportName('json'), filters:[{ name: 'JSON', extensions:['json'] }] });
        if (savePath) { await invoke('write_text_file', { path: savePath, content: JSON.stringify(data, null, 2) }); showMsg('success', t('msg_json_exported')); }
    } catch (e) { showMsg('error', `Error: ${e}`); }
}

export async function importJSON() {
    try {
        const selected = await open({ multiple: false, filters:[{ name: 'JSON', extensions: ['json'] }] });
        if (!selected) return;
        const jsonContent = await invoke('read_text_file', { path: selected });
        const data = JSON.parse(jsonContent);
        let updatedCount = 0;
        data.forEach(item => {
            const block = parsedBlocks.value.find(b => b.id === item.id);
            if (block && item.translation) { block.translation = item.translation; updatedCount++; }
        });
        if (updatedCount > 0) editorDirty.value = true;
        showMsg('success', `${t('msg_json_imported')} ${updatedCount}.`);
    } catch (e) { showMsg('error', `Error: ${e}`); }
}

// === PO (gettext) экспорт/импорт ===
// PO несёт то, чего нет в CSV: ссылку на источник (#: file:line), контекст (msgctxt — у нас
// id строки, гарантирует уникальность при одинаковых оригиналах) и флаг #, fuzzy (требует
// проверки). Совместимо с Poedit/Weblate/OmegaT/gettext.
function poEscape(s) {
    return (s || '')
        .replace(/\\/g, '\\\\').replace(/"/g, '\\"')
        .replace(/\n/g, '\\n').replace(/\t/g, '\\t').replace(/\r/g, '\\r');
}
function poUnescape(s) {
    let out = '';
    for (let i = 0; i < s.length; i++) {
        const c = s[i];
        if (c === '\\') {
            const n = s[++i];
            out += n === 'n' ? '\n' : n === 't' ? '\t' : n === 'r' ? '\r' : (n || '');
        } else { out += c; }
    }
    return out;
}
function parsePO(text) {
    const entries = [];
    let cur = null, field = null;
    const finalize = () => { if (cur && (cur.id !== undefined || cur.ctxt !== undefined)) entries.push(cur); cur = null; field = null; };
    for (const raw of text.split('\n')) {
        const line = raw.replace(/\r$/, '');
        if (line.startsWith('#')) {
            if (line.startsWith('#,') && line.includes('fuzzy')) { if (!cur) cur = {}; cur.fuzzy = true; }
            continue;
        }
        const t = line.trim();
        if (t === '') { finalize(); continue; }
        let m;
        if ((m = t.match(/^msgctxt\s+"(.*)"$/))) { if (!cur) cur = {}; cur.ctxt = poUnescape(m[1]); field = 'ctxt'; }
        else if ((m = t.match(/^msgid\s+"(.*)"$/))) { if (!cur) cur = {}; cur.id = poUnescape(m[1]); field = 'id'; }
        else if ((m = t.match(/^msgstr\s+"(.*)"$/))) { if (!cur) cur = {}; cur.str = poUnescape(m[1]); field = 'str'; }
        else if ((m = t.match(/^"(.*)"$/)) && cur) { // продолжение многострочной строки
            const v = poUnescape(m[1]);
            if (field === 'ctxt') cur.ctxt = (cur.ctxt || '') + v;
            else if (field === 'id') cur.id = (cur.id || '') + v;
            else if (field === 'str') cur.str = (cur.str || '') + v;
        }
    }
    finalize();
    return entries;
}

export async function exportPO() {
    let po = 'msgid ""\nmsgstr ""\n"Project-Id-Version: RenForge\\n"\n"Content-Type: text/plain; charset=UTF-8\\n"\n\n';
    parsedBlocks.value.forEach(b => {
        const ref = (b.file_path || '').replace(/[\n\r]/g, ' ');
        po += `#: ${ref}:${b.line_number || 0}\n`;
        if (getBlockStatus(b) === 'outdated') po += '#, fuzzy\n';
        po += `msgctxt "${poEscape(b.id)}"\n`;
        po += `msgid "${poEscape(b.original)}"\n`;
        po += `msgstr "${poEscape(b.translation)}"\n\n`;
    });
    try {
        const savePath = await save({ defaultPath: defaultExportName('po'), filters:[{ name: 'PO', extensions:['po'] }] });
        if (savePath) { await invoke('write_text_file', { path: savePath, content: po }); showMsg('success', t('msg_po_exported')); }
    } catch (e) { showMsg('error', `Error: ${e}`); }
}

export async function importPO() {
    try {
        const selected = await open({ multiple: false, filters:[{ name: 'PO', extensions: ['po', 'pot'] }] });
        if (!selected) return;
        const content = await invoke('read_text_file', { path: selected });
        const entries = parsePO(content);
        let updatedCount = 0;
        for (const e of entries) {
            if (e.ctxt === undefined) continue;       // пропускаем заголовок/строки без контекста
            if (!e.str) continue;                      // пустой перевод — не трогаем
            const block = parsedBlocks.value.find(b => b.id === e.ctxt);
            if (block) { block.translation = e.str; updatedCount++; }
        }
        if (updatedCount > 0) editorDirty.value = true;
        showMsg('success', `${t('msg_po_imported')} ${updatedCount}.`);
    } catch (e) { showMsg('error', `Error: ${e}`); }
}

function collectManualImportItems(value) {
    if (Array.isArray(value)) return value;

    if (value && typeof value === 'object') {
        if (Array.isArray(value.items)) return value.items;
        if (Array.isArray(value.strings)) return value.strings;
        if (Array.isArray(value.entries)) return value.entries;
        if (Array.isArray(value.data)) return value.data;
    }

    return [];
}

// Импорт ручных строк из JSON.
// Берёт только объекты с непустым original.
// translation добавляется только если она есть и не пустая.
// Значения НЕ trim'ятся при сохранении, чтобы не терять пробелы в начале/конце.
export async function importManualStringsJSON() {
    try {
        isProcessing.value = true;

        const selected = await open({
            multiple: false,
            filters: [{ name: 'JSON', extensions: ['json'] }],
        });

        if (!selected) return;

        const jsonContent = await invoke('read_text_file', { path: selected });
        const data = JSON.parse(jsonContent);
        const items = collectManualImportItems(data);

        if (!items.length) {
            showMsg('error', 'В JSON не найден массив строк для импорта.');
            return;
        }

        let added = 0;
        let skipped = 0;
        let lastId = null;
        const toCurrent = !!currentFilePath.value && currentFilePath.value !== MANUAL_FILE;

        for (const item of items) {
            if (!item || typeof item !== 'object') {
                skipped++;
                continue;
            }

            const original = item.original == null ? '' : String(item.original);
            if (!original.trim()) {
                skipped++;
                continue;
            }

            const translationRaw = item.translation == null ? '' : String(item.translation);
            const translation = translationRaw.trim() ? translationRaw : '';

            const id = await addManualString(original, translation, 'dialogue', toCurrent, 0);
            if (id) {
                added++;
                lastId = id;
            } else {
                skipped++;
            }
        }

        showMsg('success', `Импортировано строк: ${added}. Пропущено: ${skipped}.`);

        return { added, skipped, lastId };
    } catch (e) {
        showMsg('error', `Ошибка импорта JSON: ${e}`);
    } finally {
        isProcessing.value = false;
    }
}