<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content files-modal">
      <div class="modal-header">
        <h2><Icon name="file" :size="16" /> {{ t('files_modal_title') }}</h2>
        <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
      </div>

      <div class="fm-toolbar">
        <div class="editor-search-wrap fm-search">
          <span class="editor-search-icon"><Icon name="search" :size="14" /></span>
          <input v-model="query" :placeholder="t('files_modal_search')" class="editor-search" />
          <button v-if="query" class="editor-search-clear" @click="query = ''" :title="t('close')"><Icon name="x" :size="13" /></button>
        </div>
        <div class="segmented-control fm-filters">
          <button v-for="f in filters" :key="f.k" :class="['seg-btn', { active: filter === f.k }]" @click="filter = f.k">
            {{ t(f.label) }}<span class="fm-count" v-if="counts[f.k]"> {{ counts[f.k] }}</span>
          </button>
        </div>
      </div>

      <p class="fm-hint">{{ t('files_modal_hint') }}</p>

      <div class="fm-body">
        <div v-if="loading" class="source-status"><span class="src-spinner"></span> {{ t('loading_editor') }}</div>
        <div v-else-if="!visible.length" class="source-status">{{ t('files_modal_empty') }}</div>
        <div v-else class="fm-list">
          <button v-for="f in visible" :key="f.rel_path" class="fm-row" @click="open(f)" :title="f.rel_path">
            <span class="fm-name">
              <span class="fm-dir" v-if="dir(f.rel_path)">{{ dir(f.rel_path) }}/</span><span class="fm-leaf">{{ name(f.rel_path) }}</span>
            </span>
            <span class="fm-meta">
              <span v-if="f.status === 'extracted'" class="fm-prog">{{ f.translated }} / {{ f.total }}</span>
              <span class="fm-badge" :class="'fm-' + f.status">{{ statusLabel(f) }}</span>
            </span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { showFilesModal, showSourceModal, projectPath, getFileName, getFolderFromPath } from '../store.js';
import { openEditor } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const files = ref([]);
const loading = ref(true);
const query = ref('');
const filter = ref('all');

const filters = [
  { k: 'all', label: 'files_filter_all' },
  { k: 'empty', label: 'files_filter_empty' },
  { k: 'extracted', label: 'files_filter_extracted' },
  { k: 'lang', label: 'files_filter_lang' },
];

onMounted(async () => {
  try {
    files.value = (await invoke('list_game_files', { projectPath: projectPath.value })) || [];
  } catch (e) {
    files.value = [];
  } finally {
    loading.value = false;
  }
});

const counts = computed(() => {
  const c = { all: files.value.length, empty: 0, extracted: 0, lang: 0 };
  for (const f of files.value) c[f.status] = (c[f.status] || 0) + 1;
  return c;
});

const visible = computed(() => {
  let arr = files.value;
  if (filter.value !== 'all') arr = arr.filter((f) => f.status === filter.value);
  const q = query.value.trim().toLowerCase();
  if (q) arr = arr.filter((f) => f.rel_path.toLowerCase().includes(q));
  return arr;
});

function name(rel) { return getFileName(rel); }
function dir(rel) { const d = getFolderFromPath(rel); return d && d !== '/' ? d : ''; }
function statusLabel(f) {
  if (f.status === 'extracted') return t('files_status_extracted');
  if (f.status === 'lang') return t('files_status_lang') + (f.lang ? ` (${f.lang})` : '');
  return t('files_status_empty');
}

async function open(f) {
  showFilesModal.value = false;
  await openEditor(f.rel_path, { silent: true });
  showSourceModal.value = true;
}
function close() { showFilesModal.value = false; }
</script>

<style scoped>
.files-modal { width: 820px; max-width: 92vw; height: 78vh; display: flex; flex-direction: column; }

.fm-toolbar { display: flex; align-items: center; gap: 14px; padding: 18px 24px 10px; flex-wrap: wrap; }
.fm-search { flex: 1 1 260px; min-width: 0; }
.fm-filters { flex: 0 0 auto; }
.fm-filters .seg-btn { padding: 6px 14px; }
.fm-count { opacity: 0.7; font-variant-numeric: tabular-nums; margin-left: 3px; }
.fm-hint { font-size: 12px; color: var(--text-muted); margin: 0; padding: 0 24px 14px; line-height: 1.45; }

.fm-body { flex: 1; min-height: 0; overflow-y: auto; padding: 0 24px 22px; }
.fm-list { display: flex; flex-direction: column; gap: 7px; }
.fm-row {
  display: flex; align-items: center; justify-content: space-between; gap: 16px;
  width: 100%; text-align: left; padding: 13px 18px; border-radius: 10px;
  background: var(--bg-panel); border: 1px solid var(--border-input); cursor: pointer;
  transition: border-color .15s, background .15s;
}
.fm-row:hover { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 9%, var(--bg-panel)); }
.fm-name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px; }
.fm-dir { color: var(--text-muted); }
.fm-leaf { color: var(--text-main); font-weight: 600; }
.fm-meta { flex: 0 0 auto; display: inline-flex; align-items: center; gap: 14px; }
.fm-prog { font-size: 12px; color: var(--text-secondary); font-variant-numeric: tabular-nums; }
.fm-badge { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: .5px; padding: 4px 11px; border-radius: 999px; white-space: nowrap; }
.fm-extracted { color: #3fae6a; background: rgba(63,174,106,.15); }
.fm-empty { color: #eab308; background: rgba(234,179,8,.16); }
.fm-lang { color: var(--text-secondary); background: color-mix(in srgb, var(--text-secondary) 16%, transparent); }

.source-status { flex: 1; display: flex; align-items: center; justify-content: center; gap: 10px; padding: 40px 24px; text-align: center; color: var(--text-muted); }
.src-spinner { width: 16px; height: 16px; border: 2px solid color-mix(in srgb, var(--text-muted) 35%, transparent); border-top-color: var(--accent); border-radius: 50%; animation: spin 1s linear infinite; display: inline-block; }
</style>
