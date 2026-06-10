<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content tm-modal">
      <div class="modal-header">
        <h2><Icon name="database" :size="17" /> {{ t('tm_title') }} <span class="tm-count">{{ total }}</span></h2>
        <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
      </div>

      <div class="tm-toolbar">
        <div class="tm-search-wrap">
          <span class="tm-search-icon"><Icon name="search" :size="15" /></span>
          <input class="tm-search" type="text" v-model="query" :placeholder="t('search_placeholder')" @input="onSearch" />
          <button v-if="query" class="tm-search-clear" @click="clearSearch" :title="t('close')"><Icon name="x" :size="14" /></button>
        </div>
        <button class="btn btn-secondary tm-tool-btn" :class="{ active: showAdd }" @click="showAdd = !showAdd"><Icon name="plus" :size="14" /> {{ t('tm_add') }}</button>
        <button class="btn btn-outline tm-tool-btn tm-clear" @click="clearAll" :disabled="total === 0"><Icon name="trash" :size="14" /> {{ t('tm_clear') }}</button>
      </div>

      <transition name="tm-add-fade">
        <div v-if="showAdd" class="tm-addform">
          <input type="text" v-model="add.target_lang" :placeholder="t('target_lang')" class="tm-add-lang" />
          <input type="text" v-model="add.original" :placeholder="t('glos_orig')" />
          <input type="text" v-model="add.translation" :placeholder="t('glos_tran')" @keyup.enter="addEntry" />
          <button class="btn btn-primary tm-add-go" @click="addEntry" :disabled="!add.target_lang.trim() || !add.original.trim()"><Icon name="check" :size="14" /> {{ t('glos_add') }}</button>
        </div>
      </transition>

      <div class="modal-scroll-body tm-body">
        <div v-if="entries.length === 0" class="tm-empty">
          <Icon name="database" :size="34" :stroke-width="1.4" />
          <span>{{ query ? t('no_files_filter') : t('tm_empty') }}</span>
        </div>
        <table v-else class="tm-table">
          <thead>
            <tr>
              <th class="th-lang">{{ t('target_lang') }}</th>
              <th>{{ t('glos_orig') }}</th>
              <th>{{ t('glos_tran') }}</th>
              <th class="th-hits" :title="t('tm_hits_hint')">↺</th>
              <th class="th-del"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="e in entries" :key="e.target_lang + '\u0001' + e.original">
              <td class="tm-lang"><span class="tm-lang-chip">{{ e.target_lang }}</span></td>
              <td class="tm-orig" :title="e.original">{{ e.original }}</td>
              <td class="tm-tran">
                <input type="text" v-model="e.translation" @change="saveEntry(e)" :placeholder="t('glos_tran')" />
              </td>
              <td class="tm-hits">{{ e.hits }}</td>
              <td class="tm-del-cell">
                <button class="tm-del-btn" @click="delEntry(e)" :title="t('pairs_delete')"><Icon name="trash" :size="14" /></button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="tm-footer">
        <button class="btn btn-secondary tm-nav" @click="prev" :disabled="page === 0"><Icon name="arrow-down" :size="14" /></button>
        <span class="tm-page">{{ page + 1 }} / {{ pageCount }}</span>
        <button class="btn btn-secondary tm-nav tm-nav-next" @click="next" :disabled="(page + 1) * pageSize >= total"><Icon name="arrow-down" :size="14" /></button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { showTmModal, showMsg } from '../store.js';
import { ask } from '@tauri-apps/plugin-dialog';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const entries = ref([]);
const total = ref(0);
const query = ref('');
const page = ref(0);
const pageSize = 50;
const showAdd = ref(false);
const add = ref({ target_lang: '', original: '', translation: '' });
let searchTimer = null;

const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));

async function load() {
  try {
    const res = await invoke('tm_list', { query: query.value, limit: pageSize, offset: page.value * pageSize });
    entries.value = res.entries || [];
    total.value = res.total || 0;
  } catch (e) { showMsg('error', e.toString()); }
}
function onSearch() {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => { page.value = 0; load(); }, 250);
}
function clearSearch() { query.value = ''; page.value = 0; load(); }
function prev() { if (page.value > 0) { page.value--; load(); } }
function next() { if ((page.value + 1) * pageSize < total.value) { page.value++; load(); } }

async function saveEntry(e) {
  try {
    await invoke('tm_upsert', { targetLang: e.target_lang, original: e.original, translation: e.translation, sourceLang: e.source_lang || '' });
  } catch (err) { showMsg('error', err.toString()); }
}
async function delEntry(e) {
  try {
    await invoke('tm_delete', { targetLang: e.target_lang, original: e.original });
    if (entries.value.length === 1 && page.value > 0) page.value--;
    await load();
  } catch (err) { showMsg('error', err.toString()); }
}
async function addEntry() {
  if (!add.value.target_lang.trim() || !add.value.original.trim()) return;
  try {
    await invoke('tm_upsert', { targetLang: add.value.target_lang.trim(), original: add.value.original.trim(), translation: add.value.translation, sourceLang: '' });
    add.value = { target_lang: '', original: '', translation: '' };
    showAdd.value = false;
    page.value = 0;
    await load();
  } catch (err) { showMsg('error', err.toString()); }
}
async function clearAll() {
  const ok = await ask(t('tm_clear_confirm'), { title: t('tm_clear'), kind: 'warning' });
  if (!ok) return;
  try { await invoke('tm_clear'); page.value = 0; await load(); } catch (e) { showMsg('error', e.toString()); }
}
function close() { showTmModal.value = false; }

onMounted(load);
</script>

<style scoped>
.tm-modal { width: 820px; max-width: 94vw; display: flex; flex-direction: column; max-height: 86vh; }
.modal-header h2 { display: inline-flex; align-items: center; gap: 8px; }
.tm-count {
  font-size: 12px; font-weight: 600; line-height: 1;
  padding: 3px 9px; border-radius: 10px; margin-left: 2px;
  color: var(--accent); background: color-mix(in srgb, var(--accent) 16%, transparent);
  font-variant-numeric: tabular-nums;
}

/* Тулбар */
.tm-toolbar { display: flex; gap: 8px; align-items: center; padding: 14px 20px 12px; }
.tm-search-wrap { flex: 1; position: relative; display: flex; align-items: center; }
.tm-search-icon { position: absolute; left: 11px; color: var(--text-muted); display: inline-flex; pointer-events: none; }
.tm-search {
  width: 100%; box-sizing: border-box; padding: 8px 32px 8px 34px;
  background: var(--bg-input); border: 1px solid var(--border-input); border-radius: 8px;
  color: var(--text-main); outline: none; font-size: 13px; transition: 0.15s;
}
.tm-search:focus { border-color: var(--accent); background: var(--bg-input-focus); }
.tm-search-clear {
  position: absolute; right: 7px; display: inline-flex; align-items: center; justify-content: center;
  border: none; background: transparent; color: var(--text-muted); cursor: pointer; padding: 3px; border-radius: 5px;
}
.tm-search-clear:hover { color: var(--text-main); background: var(--bg-base); }
.tm-tool-btn { height: 36px; gap: 6px; white-space: nowrap; }
.tm-clear { color: #e05a5a; border-color: color-mix(in srgb, #e05a5a 38%, var(--border-input)); }
.tm-clear:hover:not(:disabled) { background: rgba(224,90,90,.12); border-color: #e05a5a; }

/* Форма добавления */
.tm-addform { display: flex; gap: 8px; padding: 0 20px 12px; }
.tm-addform input {
  flex: 1; padding: 8px 10px; background: var(--bg-input); border: 1px solid var(--border-input);
  border-radius: 8px; color: var(--text-main); outline: none; font-size: 13px; transition: 0.15s;
}
.tm-addform input:focus { border-color: var(--accent); background: var(--bg-input-focus); }
.tm-add-lang { max-width: 130px; }
.tm-add-go { gap: 6px; white-space: nowrap; }
.tm-add-fade-enter-active, .tm-add-fade-leave-active { transition: opacity .15s, transform .15s; }
.tm-add-fade-enter-from, .tm-add-fade-leave-to { opacity: 0; transform: translateY(-4px); }

/* Тело/таблица */
.tm-body { padding: 0 20px 4px; max-height: 56vh; }
.tm-empty {
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px;
  padding: 48px 20px; color: var(--text-muted); text-align: center;
}
.tm-table { width: 100%; border-collapse: collapse; font-size: 13px; }
.tm-table thead th {
  text-align: left; color: var(--text-secondary); font-weight: 600; font-size: 11.5px;
  text-transform: uppercase; letter-spacing: .03em;
  padding: 8px 10px; border-bottom: 1px solid var(--border-main);
  position: sticky; top: 0; background: var(--bg-app); z-index: 1;
}
.th-lang { width: 96px; }
.th-hits { width: 46px; text-align: center; }
.th-del { width: 40px; }
.tm-table tbody td { padding: 7px 10px; border-bottom: 1px solid var(--border-main); vertical-align: middle; }
.tm-table tbody tr { transition: background .12s; }
.tm-table tbody tr:hover { background: color-mix(in srgb, var(--accent) 7%, transparent); }
.tm-lang-chip {
  display: inline-block; padding: 2px 8px; border-radius: 8px; font-size: 11px; font-weight: 600;
  color: var(--accent); background: color-mix(in srgb, var(--accent) 13%, transparent); white-space: nowrap;
}
.tm-orig { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-main); }
.tm-tran input {
  width: 100%; box-sizing: border-box; padding: 6px 9px; background: var(--bg-input);
  border: 1px solid transparent; border-radius: 6px; color: var(--text-main); outline: none;
  font-size: 13px; transition: 0.15s;
}
.tm-tran input:hover { border-color: var(--border-input); }
.tm-tran input:focus { border-color: var(--accent); background: var(--bg-input-focus); }
.tm-hits { color: var(--text-muted); text-align: center; font-variant-numeric: tabular-nums; }
.tm-del-cell { text-align: center; }
.tm-del-btn {
  display: inline-flex; align-items: center; justify-content: center; padding: 5px;
  border: none; background: transparent; color: var(--text-muted); cursor: pointer; border-radius: 6px; transition: 0.15s;
}
.tm-del-btn:hover { color: #e05a5a; background: rgba(224,90,90,.12); }

/* Футер */
.tm-footer {
  display: flex; align-items: center; justify-content: center; gap: 14px;
  padding: 12px 20px; border-top: 1px solid var(--border-main); background: var(--bg-panel);
}
.tm-nav { width: 36px; height: 32px; padding: 0; }
.tm-nav :deep(.rf-icon) { transform: rotate(90deg); }
.tm-nav-next :deep(.rf-icon) { transform: rotate(-90deg); }
.tm-page { font-size: 13px; color: var(--text-secondary); font-variant-numeric: tabular-nums; min-width: 60px; text-align: center; }
</style>
