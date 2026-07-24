<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content uc-modal">
      <div class="modal-header">
        <h2><Icon name="search" :size="16" /> {{ t('uncovered_title') }}</h2>
        <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
      </div>

      <div class="uc-toolbar">
        <div class="editor-search-wrap uc-search">
          <span class="editor-search-icon"><Icon name="search" :size="14" /></span>
          <input v-model="query" :placeholder="t('files_modal_search')" class="editor-search" />
          <button v-if="query" class="editor-search-clear" @click="query = ''" :title="t('close')"><Icon name="x" :size="13" /></button>
        </div>
        <div class="segmented-control">
          <button :class="['seg-btn', { active: !showAll }]" @click="showAll = false">
            {{ t('uncov_filter_candidates') }}<span class="uc-count" v-if="candidateCount"> {{ candidateCount }}</span>
          </button>
          <button :class="['seg-btn', { active: showAll }]" @click="showAll = true">
            {{ t('files_filter_all') }}<span class="uc-count" v-if="entries.length"> {{ entries.length }}</span>
          </button>
        </div>
        <button class="btn btn-outline" @click="refresh"><Icon name="undo" :size="14" /> {{ t('uncov_refresh') }}</button>
        <button class="btn btn-outline icon-danger" @click="doClear"><Icon name="trash" :size="14" /> {{ t('uncov_clear') }}</button>
      </div>

      <p class="uc-hint">{{ t('uncovered_hint') }}</p>

      <div class="uc-body">
        <div v-if="loading" class="uc-status"><span class="uc-spinner"></span> {{ t('loading_editor') }}</div>
        <div v-else-if="!visible.length" class="uc-status">{{ t('uncov_empty') }}</div>
        <div v-else class="uc-list">
          <div v-for="(e, i) in visible" :key="i" class="uc-row">
            <span class="uc-chan" :class="'uc-chan-' + e.chan">{{ e.chan }}</span>
            <span class="uc-text" :title="e.text">{{ e.text }}</span>
            <span class="uc-actions">
              <span v-if="e.in_db" class="uc-badge">{{ t('uncov_in_db') }}</span>
              <button v-else class="btn btn-outline uc-add" @click="add(e)" :title="t('cand_add_hint')"><Icon name="plus" :size="13" /> {{ t('manual_add_btn') }}</button>
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { showUncoveredModal, projectPath, showMsg } from '../store.js';
import { addManualString } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

// Отчёт диагностики покрытия: строки, что показались в игре, но не покрыты (roadmap 0.1).
// Читаем renforge_uncovered.log через read_uncovered (бэкенд сверяет с БД: in_db/translated).
const entries = ref([]);
const loading = ref(true);
const query = ref('');
const showAll = ref(false); // false = только кандидаты (не в базе); true = всё замеченное

async function refresh() {
  loading.value = true;
  try { entries.value = (await invoke('read_uncovered', { projectPath: projectPath.value })) || []; }
  catch (e) { entries.value = []; }
  finally { loading.value = false; }
}
onMounted(refresh);

const candidateCount = computed(() => entries.value.filter(e => !e.in_db).length);
const visible = computed(() => {
  let arr = showAll.value ? entries.value : entries.value.filter(e => !e.in_db);
  const q = query.value.trim().toLowerCase();
  if (q) arr = arr.filter(e => (e.text || '').toLowerCase().includes(q));
  return arr;
});

// Добавить непокрытую строку в «Ручные строки» (перевести потом в редакторе).
async function add(e) {
  const type = e.chan === 'say' ? 'dialogue' : 'ui';
  try {
    await addManualString(e.text, '', type, false);
    e.in_db = true; // уйдёт из фильтра «кандидаты»
    showMsg('success', t('uncov_added'), 2000);
  } catch (err) { showMsg('error', err.toString()); }
}
async function doClear() {
  try { await invoke('clear_uncovered', { projectPath: projectPath.value }); entries.value = []; }
  catch (err) { showMsg('error', err.toString()); }
}
function close() { showUncoveredModal.value = false; }
</script>

<style scoped>
.uc-modal { width: 820px; max-width: 92vw; height: 78vh; display: flex; flex-direction: column; }
.uc-toolbar { display: flex; align-items: center; gap: 12px; padding: 18px 24px 10px; flex-wrap: wrap; }
.uc-search { flex: 1 1 220px; min-width: 0; }
.uc-count { opacity: .7; font-variant-numeric: tabular-nums; margin-left: 3px; }
.uc-hint { font-size: 12px; color: var(--text-muted); margin: 0; padding: 0 24px 14px; line-height: 1.45; }
.uc-body { flex: 1; min-height: 0; overflow-y: auto; padding: 0 24px 22px; }
.uc-list { display: flex; flex-direction: column; gap: 7px; }
.uc-row {
  display: flex; align-items: center; gap: 14px; width: 100%;
  padding: 11px 16px; border-radius: 10px;
  background: var(--bg-panel); border: 1px solid var(--border-input);
}
.uc-chan { flex: 0 0 auto; font-size: 9px; font-weight: 700; text-transform: uppercase; letter-spacing: .5px; padding: 3px 8px; border-radius: 6px; color: var(--text-secondary); background: color-mix(in srgb, var(--text-muted) 14%, transparent); min-width: 48px; text-align: center; }
.uc-chan-say { color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }
.uc-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px; color: var(--text-main); }
.uc-actions { flex: 0 0 auto; display: inline-flex; align-items: center; gap: 10px; }
.uc-badge { font-size: 10px; color: var(--text-muted); background: color-mix(in srgb, var(--text-muted) 14%, transparent); padding: 4px 10px; border-radius: 999px; white-space: nowrap; }
.uc-add { padding: 5px 12px; }
.uc-status { flex: 1; display: flex; align-items: center; justify-content: center; gap: 10px; padding: 40px 24px; text-align: center; color: var(--text-muted); line-height: 1.5; }
.uc-spinner { width: 16px; height: 16px; border: 2px solid color-mix(in srgb, var(--text-muted) 35%, transparent); border-top-color: var(--accent); border-radius: 50%; animation: spin 1s linear infinite; display: inline-block; }
</style>
