<template>
  <div class="dashboard">
    <div v-if="!projectPath" class="empty-state">
      <EmptyState icon="folder" :title="t('empty_state_title')" :hint="t('empty_state_desc1')">
        <p class="empty-hint">{{ t('empty_state_desc2') }}</p>
      </EmptyState>
    </div>

    <div v-else class="dashboard-content">
      <!-- ИНФО ОБ ИГРЕ -->
      <div class="game-info-bar">
        <span class="gi-name" :title="gameName || folderName">{{ gameName || folderName }}</span>
        <span class="gi-tag" v-if="gameVersion">v{{ gameVersion }}</span>
        <span class="gi-tag gi-engine" v-if="engineVersion">Ren'Py {{ engineVersion }}</span>
        <span class="gi-tag gi-lang" :title="targetLang || t('lang_not_set')">→ {{ targetLang || t('lang_not_set') }}</span>
        <button class="btn btn-secondary gi-update-btn" @click="showUpdateModal = true" :title="t('update_title')"><Icon name="undo" :size="15" /> {{ t('update_btn') }}</button>
      </div>

      <!-- ПРЕДУПРЕЖДЕНИЕ О МУЛЬТИЯЗЫЧНОЙ КОЛЛИЗИИ (roadmap 1.2) -->
      <div v-if="targetLangCollision" class="lang-collision-bar">
        <Icon name="alert" :size="16" />
        <span>{{ collisionMsg }}</span>
      </div>

      <!-- РАБОЧИЕ ПРОСТРАНСТВА (ПАРЫ ЯЗЫКОВ) -->
      <PairsWidget />

      <!-- ПАМЯТЬ ПЕРЕВОДОВ (TM) -->
      <div class="tm-bar">
        <div class="tm-bar-info">
          <span class="tm-bar-icon"><Icon name="database" :size="16" /></span>
          <div class="tm-bar-text">
            <span class="tm-bar-label">{{ t('tm_title') }}<span v-if="tmCount > 0" class="tm-bar-count">{{ tmCount }}</span></span>
            <span class="tm-bar-sub">{{ t('tm_fill_hint') }}</span>
          </div>
        </div>
        <div class="tm-bar-actions">
          <button class="btn btn-outline tm-bar-btn" :disabled="!extracted || isProcessing" @click="doTmFill" :title="t('tm_fill_hint')"><Icon name="download" :size="14" /> {{ t('tm_fill') }}</button>
          <button class="btn btn-secondary tm-bar-btn" @click="showTmModal = true"><Icon name="edit" :size="14" /> {{ t('tm_manage') }}</button>
        </div>
      </div>

      <!-- ПОИСК ПО СТРОКАМ -->
      <div class="search-section">
        <input type="text" v-model="searchQuery" @input="handleSearch" :placeholder="t('search_placeholder')" class="search-input" />
        <div v-if="searchResults.length > 0" class="search-results">
          <div v-for="res in searchResults" :key="res.id" class="search-res-item" @click="jumpToFile(res)">
            <span class="res-file">{{ getFileName(res.file_path) }}</span>
            <span class="res-text"><b>{{ res.original }}</b></span>
            <span class="res-tran">{{ res.translation || '...' }}</span>
          </div>
        </div>
      </div>

      <!-- ПАЙПЛАЙН -->
      <div class="pipeline">
        <div class="pipe-step" :class="'pipe-' + stepState(1)">
          <div class="pipe-badge"><Icon v-if="extracted" name="check" :size="18" :stroke-width="3" /><span v-else>1</span></div>
          <div class="pipe-body">
            <div class="pipe-title">{{ t('extract_title') }}</div>
            <div class="pipe-sub">{{ extracted ? overall.total + ' ' + t('strings_word') : (langsReady ? t('pipe_extract_sub') : t('msg_pick_langs')) }}</div>
          </div>
          <button class="btn btn-primary pipe-btn" :disabled="isProcessing || !langsReady" :title="langsReady ? '' : t('msg_pick_langs')" @click="doExtract">
            {{ extracted ? t('re_extract') : t('extract_btn') }}
          </button>
        </div>

        <div class="pipe-arrow" :class="{ 'pipe-arrow-on': extracted }"></div>

        <div class="pipe-step" :class="'pipe-' + stepState(2)">
          <div class="pipe-badge"><Icon v-if="extracted && overall.pct >= 100" name="check" :size="18" :stroke-width="3" /><span v-else>2</span></div>
          <div class="pipe-body">
            <div class="pipe-title">{{ t('translate_title') }}</div>
            <div class="pipe-sub">{{ extracted ? overall.pct + '% • ' + overall.tr + '/' + overall.total : t('pipe_locked') }}</div>
          </div>
        </div>

        <div class="pipe-arrow" :class="{ 'pipe-arrow-on': extracted && overall.pct >= 100 }"></div>

        <div class="pipe-step" :class="'pipe-' + stepState(3)">
          <div class="pipe-badge"><span>3</span></div>
          <div class="pipe-body">
            <div class="pipe-title">{{ t('build_title') }}</div>
            <div class="pipe-sub">{{ extracted ? t('gen_patch') : t('pipe_locked') }}</div>
          </div>
          <div class="pipe-actions">
            <button class="btn btn-secondary icon-only-btn" @click="showDeliveryHooksModal = true" :title="t('hooks_title')"><Icon name="code" :size="16" /></button>
            <button class="btn btn-secondary icon-only-btn" :class="{ active: diagnosticBuild }" @click="diagnosticBuild = !diagnosticBuild" :title="t('diag_build')"><Icon name="eye" :size="16" /></button>
            <button class="btn btn-secondary icon-only-btn" @click="showUncoveredModal = true" :title="t('uncovered_title')"><Icon name="search" :size="16" /></button>
            <button class="btn btn-secondary icon-only-btn" :class="{ active: showFontPanel }" @click="showFontPanel = !showFontPanel" :title="t('fonts')"><Icon name="font" :size="16" /></button>
            <button class="btn btn-primary pipe-btn" :disabled="!extracted || isProcessing" @click="doBuildMod">{{ t('build_mod_btn') }}</button>
          </div>
        </div>
      </div>

      <!-- ПАНЕЛЬ ШРИФТОВ -->
      <div v-if="showFontPanel" class="font-settings-panel">
        <div class="font-panel-head">
          <strong>{{ t('fonts_title') }}</strong>
          <p>{{ t('fonts_hint') }}</p>
        </div>
        <div v-if="projectFonts.length" class="font-list">
          <div v-for="f in projectFonts" :key="f.rel_path" class="font-row" :title="f.rel_path">
            <span class="font-name">{{ f.name }}</span>
            <span class="font-scripts">
              <span v-for="s in f.scripts" :key="s" class="font-cyr cyr-yes" :class="{ 'is-target': s === targetScript }">{{ scriptLabel(s) }} ✓</span>
              <span v-if="targetScript && !f.scripts.includes(targetScript)" class="font-cyr cyr-no is-target">{{ scriptLabel(targetScript) }} ✗</span>
            </span>
            <select class="settings-select font-target-select" v-model="f.mode" @change="onFontMode(f)">
              <option value="keep">{{ t('font_keep') }}</option>
              <option value="default">{{ t('font_default_dejavu') }}</option>
              <option value="custom">{{ f.mode === 'custom' && f.targetName ? f.targetName : t('font_custom') }}</option>
            </select>
          </div>
        </div>
        <div v-else class="font-empty">{{ extracted ? t('no_fonts_found') : t('pipe_locked') }}</div>
      </div>

      <!-- СТАТ-КАРТОЧКИ -->
      <div class="stat-cards">
        <div class="stat-card">
          <span class="stat-wm"><Icon name="translate" :size="124" :stroke-width="1.6" /></span>
          <div class="stat-body">
            <div class="ring" :style="{ background: `conic-gradient(var(--accent) ${overall.pct}%, var(--border-main) 0)` }"><span>{{ overall.pct }}%</span></div>
            <div class="stat-meta">
              <div class="stat-label">{{ t('stat_translated') }}</div>
              <div class="stat-sub">{{ overall.tr }} / {{ overall.total }} {{ t('strings_word') }}</div>
            </div>
          </div>
        </div>
        <div class="stat-card">
          <span class="stat-wm"><Icon name="file" :size="124" :stroke-width="1.6" /></span>
          <div class="stat-body">
            <div class="stat-num">{{ counts.all }}</div>
            <div class="stat-meta">
              <div class="stat-label">{{ t('stat_files') }}</div>
              <div class="stat-sub">{{ counts.done }} {{ t('stat_done') }}</div>
              <div class="stat-hidden-note" v-if="hiddenCounts.files > 0"><Icon name="eye-off" :size="11" /> {{ hiddenCounts.files }} {{ t('stat_hidden') }}</div>
            </div>
          </div>
        </div>
        <div class="stat-card">
          <span class="stat-wm"><Icon name="image" :size="124" :stroke-width="1.6" /></span>
          <div class="stat-body">
            <div class="stat-num">{{ imgStat ? imgStat.loc : '—' }}<span class="stat-of">/ {{ imgStat ? imgStat.total : '—' }}</span></div>
            <div class="stat-meta">
              <div class="stat-label">{{ t('stat_images') }}</div>
              <div class="stat-sub">{{ t('stat_localized') }}</div>
              <div class="stat-hidden-note" v-if="hiddenCounts.images > 0"><Icon name="eye-off" :size="11" /> {{ hiddenCounts.images }} {{ t('stat_hidden') }}</div>
            </div>
          </div>
        </div>
        <div class="stat-card">
          <span class="stat-wm"><Icon name="music" :size="124" :stroke-width="1.6" /></span>
          <div class="stat-body">
            <div class="stat-num">{{ audStat ? audStat.loc : '—' }}<span class="stat-of">/ {{ audStat ? audStat.total : '—' }}</span></div>
            <div class="stat-meta">
              <div class="stat-label">{{ t('stat_audio') }}</div>
              <div class="stat-sub">{{ t('stat_localized') }}</div>
              <div class="stat-hidden-note" v-if="hiddenCounts.audio > 0"><Icon name="eye-off" :size="11" /> {{ hiddenCounts.audio }} {{ t('stat_hidden') }}</div>
            </div>
          </div>
        </div>
      </div>

      <!-- СПИСОК ФАЙЛОВ -->
      <div class="files-panel">
        <div class="files-toolbar">
          <div class="filter-chips">
            <button class="chip" :class="{ active: fileFilter === 'all' }" @click="fileFilter = 'all'">{{ t('f_all') }} <b>{{ counts.all }}</b></button>
            <button class="chip" :class="{ active: fileFilter === 'todo' }" @click="fileFilter = 'todo'">{{ t('f_todo') }} <b>{{ counts.todo }}</b></button>
            <button class="chip" :class="{ active: fileFilter === 'progress' }" @click="fileFilter = 'progress'">{{ t('f_progress') }} <b>{{ counts.prog }}</b></button>
            <button class="chip" :class="{ active: fileFilter === 'done' }" @click="fileFilter = 'done'">{{ t('f_done') }} <b>{{ counts.done }}</b></button>
          </div>
          <div class="files-toolbar-right">
            <button class="btn btn-secondary" @click="openEditor(MANUAL_FILE)" :disabled="isProcessing" :title="t('manual_strings_hint')" style="display:inline-flex; align-items:center; gap:5px;"><Icon name="plus" :size="14" /> {{ t('manual_strings_file') }}</button>
            <button class="btn btn-secondary" @click="showFilesModal = true" :disabled="isProcessing" :title="t('open_source_file_hint')" style="display:inline-flex; align-items:center; gap:5px;"><Icon name="file" :size="14" /> {{ t('open_source_file') }}</button>
            <label class="toggle-hidden" v-if="hiddenFiles.length > 0" style="margin:0; font-size:12px;"><input type="checkbox" v-model="showHidden"> {{ t('show_hidden') }}</label>
            <select class="settings-select" v-model="fileSort" style="width: auto; padding: 5px 8px; font-size: 12px;">
              <option value="name">{{ t('sort_name') }}</option>
              <option value="lines">{{ t('sort_lines') }}</option>
              <option value="progress">{{ t('sort_progress') }}</option>
            </select>
            <button class="btn btn-secondary icon-only-btn" @click="sortDir = sortDir === 'asc' ? 'desc' : 'asc'" :title="sortDir === 'asc' ? t('sort_asc') : t('sort_desc')">{{ sortDir === 'asc' ? '↑' : '↓' }}</button>
          </div>
        </div>

        <div class="files-scroll">
          <div class="file-row" v-for="file in visibleFiles" :key="file" :class="{ 'is-hidden': hiddenFiles.includes(file) }">
            <div class="file-main">
              <div class="file-row-top">
                <span class="file-pathline" :title="file"><span class="file-dir" v-if="file !== MANUAL_FILE && fileDir(file) && fileDir(file) !== '/'">{{ fileDir(file) }}/</span><span class="file-name">{{ file === MANUAL_FILE ? t('manual_strings_file') : getFileName(file) }}</span></span>
                <span class="status-badge" :class="badgeClass(file)">{{ badgeText(file) }}</span>
                <span class="status-badge badge-review" v-if="fileStats[file] && fileStats[file].outdated > 0">{{ t('needs_review_tag') }}</span>
                <span class="file-note" v-if="fileNotes[file] && editingNote !== file" @click.stop="startEditNote(file)" :title="t('file_note')">{{ fileNotes[file] }}</span>
                <input v-if="editingNote === file" class="file-note-input" :ref="el => noteInputRef = el"
                       v-model="fileNotes[file]" :placeholder="t('note_placeholder')"
                       @click.stop @keyup.enter="editingNote = null" @blur="editingNote = null" />
              </div>
              <div class="file-row-bottom" v-if="fileStats[file]">
                <div class="progress-bar-bg"><div class="progress-bar-fill" :style="{ width: filePct(file) * 100 + '%' }"></div></div>
                <span class="file-pct">{{ fileStats[file].translated }} / {{ fileStats[file].total }}</span>
              </div>
              <div class="file-row-bottom" v-else><span class="file-pct">{{ t('no_strings_db') }}</span></div>
            </div>
            <div class="card-actions file-row-actions">
              <button class="icon-text-btn" @click="startEditNote(file)" :title="t('file_note')"><Icon name="edit" /></button>
              <button class="icon-text-btn" @click="toggleHide(file)" :title="hiddenFiles.includes(file) ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenFiles.includes(file) ? 'eye-off' : 'eye'" /></button>
            </div>
            <button class="btn btn-primary file-tr-btn" @click="openEditor(file)" :disabled="isProcessing">{{ t('btn_translate') }}</button>
          </div>

          <div v-if="visibleFiles.length === 0" class="no-files">
            <EmptyState :icon="extracted ? 'search' : 'file'" :title="extracted ? t('no_files_filter') : t('no_files_extract')" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { t } from '../locales.js';
import Icon from './Icon.vue';
import PairsWidget from './PairsWidget.vue';
import EmptyState from './EmptyState.vue';
import {
  projectPath, isProcessing, targetLang, sourceLang, targetScript as targetScriptSetting, hiddenFiles, completedFiles,
  showHidden, showFontPanel, getFileName, getFolderFromPath, fileStats, MANUAL_FILE, showDeliveryHooksModal,
  showSourceModal, showFilesModal, showUncoveredModal, diagnosticBuild, targetLangCollision,
  showMsg, searchQuery, searchResults, fileNotes, showUpdateModal, showTmModal,
  scrollToBlock,
  hiddenImages, hiddenAudio, hiddenFolders
} from '../store.js';
import { prepareProject, buildMod, openEditor, tmFill } from '../actions.js';

const projectFonts = ref([]);
const langsReady = computed(() => !!sourceLang.value && !!targetLang.value);
// Текст предупреждения о мультиязычной коллизии (roadmap 1.2): подставляем имя языка в {lang}.
const collisionMsg = computed(() => t('lang_collision_warn').replace(/\{lang\}/g, targetLang.value));

// Память переводов: показываем размер базы рядом с заголовком панели.
const tmCount = ref(0);
async function loadTmCount() {
  try { tmCount.value = await invoke('tm_count'); } catch (e) { tmCount.value = 0; }
}
async function doTmFill() {
  await tmFill();
  loadTmCount();
}
// Перечитываем счётчик при закрытии редактора TM (могли добавить/удалить записи).
watch(showTmModal, (v) => { if (!v) loadTmCount(); });

// Заметки к файлам (редактирование инлайн)
const editingNote = ref(null);
let noteInputRef = null;
function startEditNote(file) {
  if (fileNotes.value[file] === undefined) fileNotes.value[file] = '';
  editingNote.value = file;
  nextTick(() => { if (noteInputRef) noteInputRef.focus(); });
}

// Сопоставление целевого языка с письменностью (для выбора дефолтов).
function langToScript(lang) {
  const l = (lang || '').toLowerCase();
  if (/(russ|ukrain|belarus|bulgar|serb|maced|kazakh|kyrg|mongol|русск|україн)/.test(l)) return 'cyrillic';
  if (/(japan|nihongo|япон)/.test(l)) return 'japanese';
  if (/(chin|mandar|simplified|traditional|hans|hant|кита)/.test(l)) return 'chinese';
  if (/(korea|hangul|коре)/.test(l)) return 'korean';
  if (/(arab|farsi|persian|urdu|pashto|араб|перс)/.test(l)) return 'arabic';
  if (/(thai|тайс)/.test(l)) return 'thai';
  if (/(hebrew|ivrit|иврит)/.test(l)) return 'hebrew';
  if (/(greek|hellen|греч)/.test(l)) return 'greek';
  if (/(viet|вьетнам)/.test(l)) return 'vietnamese';
  if (/(armenian|армян)/.test(l)) return 'armenian';
  if (/(georgian|груз)/.test(l)) return 'georgian';
  if (/(hindi|marathi|nepali|sanskrit|хинди|непали|санскрит)/.test(l)) return 'devanagari';
  if (/(bengali|bangla|бенгал)/.test(l)) return 'bengali';
  if (/(punjabi|panjabi|gurmukhi|панджаб|пенджаб)/.test(l)) return 'gurmukhi';
  if (/(gujarati|гуджарат)/.test(l)) return 'gujarati';
  if (/(tamil|тамил)/.test(l)) return 'tamil';
  if (/(telugu|телугу)/.test(l)) return 'telugu';
  if (/(kannada|каннада)/.test(l)) return 'kannada';
  if (/(malayalam|малаялам)/.test(l)) return 'malayalam';
  if (/(sinhal|сингал)/.test(l)) return 'sinhala';
  if (/(\blao\b|laotian|лаос)/.test(l)) return 'lao';
  if (/(tibet|тибет)/.test(l)) return 'tibetan';
  if (/(burmese|myanmar|бирман|мьянм)/.test(l)) return 'myanmar';
  if (/(khmer|cambod|кхмер|камбодж)/.test(l)) return 'khmer';
  if (/(amharic|ethiop|tigrinya|амхар|эфиоп)/.test(l)) return 'ethiopic';
  return 'latin';
}
const targetScript = computed(() =>
  targetScriptSetting.value && targetScriptSetting.value !== 'auto'
    ? targetScriptSetting.value
    : langToScript(targetLang.value)
);
// Скрипты, которые покрывает встроенный DejaVu Sans (его можно ставить дефолтом).
const DEJAVU_SCRIPTS = ['latin', 'vietnamese', 'cyrillic', 'greek', 'armenian', 'georgian', 'hebrew', 'arabic'];
function scriptLabel(code) { return t('script_' + code); }

const fileFilter = ref('all'); // all | todo | progress | done
const fileSort = ref('name');  // name | lines | progress
const sortDir = ref('asc');    // asc | desc

const imgListRaw = ref(null);
const audListRaw = ref(null);

// Видимые (не скрытые) медиа — скрытые элементы и скрытые папки в зачёт не идут.
function mediaStat(list, hiddenList) {
  if (!list) return null;
  const visible = list.filter(m =>
    !hiddenList.value.includes(m.rel_path) &&
    !hiddenFolders.value.includes(getFolderFromPath(m.rel_path))
  );
  return { total: visible.length, loc: visible.filter(m => m.is_translated).length };
}
const imgStat = computed(() => mediaStat(imgListRaw.value, hiddenImages));
const audStat = computed(() => mediaStat(audListRaw.value, hiddenAudio));

// Сколько элементов скрыто по каждому классу — показываем отдельной строкой в его карточке.
// Скрытое в общий прогресс/счётчики не входит (см. overall и mediaStat).
const hiddenCounts = computed(() => {
  const files = Object.keys(fileStats.value).filter(f => hiddenFiles.value.includes(f)).length;
  const countHidden = (list, hiddenList) => {
    if (!list) return 0;
    return list.filter(m =>
      hiddenList.value.includes(m.rel_path) ||
      hiddenFolders.value.includes(getFolderFromPath(m.rel_path))
    ).length;
  };
  return {
    files,
    images: countHidden(imgListRaw.value, hiddenImages),
    audio: countHidden(audListRaw.value, hiddenAudio),
  };
});

// Инфо об игре (из project_meta)
const gameName = ref('');
const gameVersion = ref('');
const engineVersion = ref('');
const folderName = computed(() => getFileName((projectPath.value || '').replace(/[\\/]+$/, '')));

async function loadGameMeta() {
  if (!projectPath.value) return;
  try {
    const [n, v, e] = await Promise.all([
      invoke('get_project_meta', { projectPath: projectPath.value, key: 'game_name' }),
      invoke('get_project_meta', { projectPath: projectPath.value, key: 'game_version' }),
      invoke('get_project_meta', { projectPath: projectPath.value, key: 'engine_version' }),
    ]);
    gameName.value = n || ''; gameVersion.value = v || ''; engineVersion.value = e || '';
  } catch (e) { /* нет данных — покажем имя папки */ }
}

// --- агрегаты ---
// Общий прогресс перевода. Скрытые файлы — отдельный класс: в общий зачёт НЕ идут
// (независимо от тумблера «Показывать скрытые», который влияет лишь на список файлов).
const overall = computed(() => {
  let total = 0, tr = 0;
  for (const f in fileStats.value) {
    if (hiddenFiles.value.includes(f)) continue;
    total += fileStats.value[f].total || 0;
    tr += fileStats.value[f].translated || 0;
  }
  return { total, tr, pct: total ? Math.round((tr / total) * 100) : 0 };
});

const extracted = computed(() => Object.keys(fileStats.value).length > 0);

function stepState(n) {
  if (n === 1) return extracted.value ? 'done' : 'active';
  if (n === 2) return !extracted.value ? 'locked' : (overall.value.pct >= 100 ? 'done' : 'active');
  return !extracted.value ? 'locked' : 'active';
}

function filePct(f) {
  const s = fileStats.value[f];
  return s && s.total ? s.translated / s.total : 0;
}
function fileLines(f) {
  const s = fileStats.value[f];
  return s ? (s.total || 0) : 0;
}
function fileDone(f) { return completedFiles.value.includes(f) || filePct(f) >= 1; }
function fileDir(f) { return getFolderFromPath(f); }

function badgeText(f) {
  if (fileDone(f)) return t('f_done');
  if (filePct(f) > 0) return t('f_progress');
  return t('f_todo');
}
function badgeClass(f) {
  if (fileDone(f)) return 'badge-done';
  if (filePct(f) > 0) return 'badge-prog';
  return 'badge-todo';
}

const baseFiles = computed(() => {
  let files = Object.keys(fileStats.value);
  if (!showHidden.value) files = files.filter(f => !hiddenFiles.value.includes(f));
  return files;
});

const counts = computed(() => {
  let all = 0, done = 0, prog = 0, todo = 0;
  baseFiles.value.forEach(f => {
    all++;
    if (fileDone(f)) done++;
    else if (filePct(f) > 0) prog++;
    else todo++;
  });
  return { all, done, prog, todo };
});

const visibleFiles = computed(() => {
  let files = baseFiles.value.filter(f => {
    if (fileFilter.value === 'done') return fileDone(f);
    if (fileFilter.value === 'progress') return !fileDone(f) && filePct(f) > 0;
    if (fileFilter.value === 'todo') return !fileDone(f) && filePct(f) === 0;
    return true;
  }).slice();

  if (fileSort.value === 'progress') files.sort((a, b) => filePct(a) - filePct(b));
  else if (fileSort.value === 'lines') files.sort((a, b) => fileLines(a) - fileLines(b));
  else files.sort((a, b) => a.localeCompare(b));

  if (sortDir.value === 'desc') files.reverse();
  return files;
});

async function loadAssetStats() {
  if (!projectPath.value) return;
  try {
    imgListRaw.value = await invoke('get_images_list', { projectPath: projectPath.value, targetLang: targetLang.value });
  } catch (e) { imgListRaw.value = null; }
  try {
    audListRaw.value = await invoke('get_audio_list', { projectPath: projectPath.value, targetLang: targetLang.value });
  } catch (e) { audListRaw.value = null; }
}

async function handleSearch() {
  if (searchQuery.value.length < 3) { searchResults.value = []; return; }
  try { searchResults.value = await invoke('search_in_db', { projectPath: projectPath.value, query: searchQuery.value }); } catch (e) { console.error(e); }
}

async function jumpToFile(result) {
  await openEditor(result.file_path);
  searchQuery.value = ''; searchResults.value = [];
  // Даём редактору отрисовать блоки, затем центрируемся с финализацией высот.
  setTimeout(() => scrollToBlock(result.id), 500);
}

// Дефолтный режим для шрифта по текущему целевому языку:
// шрифт уже умеет нужный скрипт → оставить; DejaVu закроет (lat/cyr/greek) → встроенный;
// иначе (CJK и пр.) DejaVu не поможет → оставить, пусть юзер выберет свой файл.
function defaultModeFor(f) {
  const ts = targetScript.value;
  if (f.scripts.includes(ts)) return 'keep';
  if (DEJAVU_SCRIPTS.includes(ts)) return 'default';
  return 'keep';
}

// Пользователь выбрал режим подмены для конкретного шрифта.
// 'custom' — открываем диалог выбора своего файла; отмена → возврат к дефолту.
async function onFontMode(f) {
  if (f.mode !== 'custom') {
    f.targetPath = ''; f.targetName = '';
    return;
  }
  try {
    const selected = await openDialog({ multiple: false, filters: [{ name: 'Fonts', extensions: ['ttf', 'otf', 'woff', 'woff2'] }] });
    if (selected) {
      f.targetPath = selected;
      f.targetName = getFileName(selected);
    } else {
      // отмена — откатываем выбор
      f.mode = f.defaultMode;
      f.targetPath = ''; f.targetName = '';
    }
  } catch (e) {
    showMsg('error', e.toString());
    f.mode = f.defaultMode;
  }
}

async function doBuildMod() {
  const fontRemaps = projectFonts.value
    .filter(f => f.mode !== 'keep')
    .filter(f => !(f.mode === 'custom' && !f.targetPath))
    .map(f => ({ source: f.rel_path, target: f.mode === 'custom' ? f.targetPath : null }));
  await buildMod(fontRemaps);
  loadAssetStats();
}

async function loadProjectFonts() {
  if (!projectPath.value || !extracted.value) { projectFonts.value = []; return; }
  try {
    const list = await invoke('get_project_fonts', { projectPath: projectPath.value });
    projectFonts.value = list.map(f => {
      const dm = defaultModeFor(f);
      return { ...f, mode: dm, defaultMode: dm, targetPath: '', targetName: '' };
    });
  } catch (e) { projectFonts.value = []; }
}

async function doExtract() {
  await prepareProject();
  // после извлечения БД пополнилась — перечитываем мету и статистику
  loadGameMeta();
  loadAssetStats();
}

function toggleHide(filePath) {
  if (hiddenFiles.value.includes(filePath)) hiddenFiles.value = hiddenFiles.value.filter(p => p !== filePath);
  else hiddenFiles.value.push(filePath);
}

watch(extracted, (v) => { if (v) { loadAssetStats(); loadGameMeta(); loadProjectFonts(); } });
watch(projectPath, () => { loadGameMeta(); loadAssetStats(); loadProjectFonts(); });
watch(targetLang, () => { loadProjectFonts(); });
watch(targetScriptSetting, () => { loadProjectFonts(); });
watch(showFontPanel, (v) => { if (v && projectFonts.value.length === 0) loadProjectFonts(); });
onMounted(() => { loadAssetStats(); loadGameMeta(); loadProjectFonts(); loadTmCount(); });
</script>

<style scoped>
.lang-collision-bar {
  display: flex; align-items: flex-start; gap: 10px;
  margin: 0 0 16px; padding: 11px 16px; border-radius: 10px;
  background: color-mix(in srgb, #eab308 12%, var(--bg-panel));
  border: 1px solid color-mix(in srgb, #eab308 45%, transparent);
  color: var(--text-secondary); font-size: 12.5px; line-height: 1.45;
}
.lang-collision-bar :deep(svg) { color: #eab308; flex: 0 0 auto; margin-top: 1px; }
.tm-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 11px 14px;
  background: var(--bg-panel);
  border: 1px solid var(--border-main);
  border-radius: 10px;
  flex-wrap: wrap;
}
.tm-bar-info { display: inline-flex; align-items: center; gap: 11px; min-width: 0; }
.tm-bar-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px; height: 34px;
  border-radius: 8px;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  flex-shrink: 0;
}
.tm-bar-text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
.tm-bar-label { font-size: 13px; font-weight: 600; color: var(--text-main); display: inline-flex; align-items: center; gap: 7px; }
.tm-bar-count {
  font-size: 11px; font-weight: 600; line-height: 1;
  padding: 2px 7px; border-radius: 9px;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  font-variant-numeric: tabular-nums;
}
.tm-bar-sub { font-size: 11.5px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 520px; }
.tm-bar-actions { display: inline-flex; gap: 8px; flex-shrink: 0; }
.tm-bar-btn { height: 34px; gap: 6px; }

.stat-hidden-note {
  display: inline-flex; align-items: center; gap: 4px;
  margin-top: 3px; font-size: 12.5px; color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.stat-hidden-note :deep(.rf-icon) { opacity: .8; }

/* Фоновый водяной знак карточки прогресса */
.stat-card { position: relative; overflow: hidden; }
.stat-wm {
  position: absolute;
  right: 8px; top: 50%;
  transform: translateY(-50%);
  display: inline-flex;
  color: var(--text-main);
  opacity: .07;
  pointer-events: none;
  z-index: 0;
}
.stat-card .stat-body { position: relative; z-index: 1; }
.stat-card .ring,
.stat-card .stat-num,
.stat-card .stat-meta { position: relative; z-index: 1; }
</style>

