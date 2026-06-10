<template>
  <div class="gallery-workspace">
      <aside class="sidebar media-sidebar">
          <div class="sidebar-title">{{ t('folders') }}</div>
          <div class="sidebar-list">
              <div class="sidebar-item media-folder-item" :class="{ active: audioSelectedFolder === '' }" @click="audioSelectedFolder = ''">
                  <span class="folder-name">{{ t('all_folders') }}</span>
              </div>
              <div class="sidebar-item media-folder-item" :class="{ active: audioSelectedFolder === '/', 'is-faded': hiddenFolders.includes('/') }" @click="audioSelectedFolder = '/'">
                  <span class="folder-name">{{ t('root_folder') }}</span>
                  <button class="icon-text-btn folder-eye-btn" @click.stop="toggleHideFolder('/')" :title="hiddenFolders.includes('/') ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenFolders.includes('/') ? 'eye-off' : 'eye'" /></button>
              </div>
              <div class="sidebar-item media-folder-item" v-for="f in audioFolders" :key="f" :class="{ active: audioSelectedFolder === f, 'is-faded': hiddenFolders.includes(f) }" @click="audioSelectedFolder = f">
                  <span class="folder-name" :title="f">{{ f }}</span>
                  <button class="icon-text-btn folder-eye-btn" @click.stop="toggleHideFolder(f)" :title="hiddenFolders.includes(f) ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenFolders.includes(f) ? 'eye-off' : 'eye'" /></button>
              </div>
          </div>
      </aside>

      <main class="media-main">
          <div class="gallery-header">
              <h2>{{ t('audio') }} <span style="color: var(--text-muted); font-weight: normal; font-size: 14px; margin-left: 5px;">› {{ audioSelectedFolder === '' ? t('all_folders') : audioSelectedFolder }}</span></h2>
              <div class="gallery-actions" style="display: flex; gap: 15px;">
                  <label class="toggle-hidden" style="margin: 0; align-items: center; display: flex;" v-if="hiddenAudio.length > 0 || hiddenFolders.length > 0">
                    <input type="checkbox" v-model="showHiddenMedia">
                    {{ t('show_hidden') }}
                  </label>
                  <input type="text" v-model="audioSearch" :placeholder="t('search_placeholder')" class="search-input" style="width: 250px; padding: 8px 15px;"/>
              </div>
          </div>

          <div v-if="isAudioLoading" class="gallery-loading" style="padding: 30px; text-align: center;">
              <p>{{ t('loading_audio') }}</p>
          </div>

          <div v-else class="gallery-scroll-container">
              <div class="gallery-grid" style="grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));">
                  <div class="gallery-card" v-for="aud in paginatedAudio" :key="aud.rel_path" style="min-height: auto;" :data-relpath="aud.rel_path" :class="{ 'is-hidden': hiddenAudio.includes(aud.rel_path), 'drag-over': dragOverPath === aud.rel_path }">
                      <div style="padding: 15px 15px 10px; background: var(--bg-panel); border-bottom: 1px solid var(--border-main); text-align: center; position: relative;">
                          <audio controls controlsList="nodownload" :src="getAudioSrc(aud)" @play="onAudioPlay" style="width: 100%; height: 36px; outline: none;"></audio>
                          <div v-if="aud.is_translated" class="status-badge status-done img-badge" style="position: absolute; top: 10px; right: 10px; margin: 0;">{{ t('status_translated') }}</div>
                      </div>
                      <div style="padding: 12px; background: var(--bg-app); border-bottom: 1px solid var(--border-main); flex: 1; display: flex; flex-direction: column;">
                          <div v-if="aud.mapped_text" style="display: flex; flex-direction: column; gap: 8px; flex: 1; justify-content: center;">
                              <div style="font-size: 13px; font-weight: 500; color: var(--text-main); font-style: italic; background: var(--bg-base); padding: 8px 10px; border-radius: 6px; border-left: 3px solid var(--accent); line-height: 1.4;">
                                  {{ aud.mapped_text }}
                              </div>
                              <div style="font-size: 11px; color: var(--text-muted); font-weight: 600; display: flex; align-items: center; gap: 5px; min-width: 0;" :title="aud.mapped_script"><Icon name="file" :size="12" /> <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ aud.mapped_script }}</span></div>
                          </div>
                          <div v-else style="display: flex; align-items: center; justify-content: center; height: 100%; opacity: 0.6;">
                              <span style="font-size: 12px; color: var(--text-muted); font-style: italic;">{{ t('audio_unmapped') }}</span>
                          </div>
                      </div>
                      <div class="gallery-card-info" style="border-top: none;">
                          <div class="img-path" style="margin-bottom: 4px;" :title="aud.rel_path">{{ aud.rel_path }}</div>
                          <div class="card-actions">
                              <button class="icon-text-btn import-btn" @click="importAudioDialog(aud)" :title="`${t('btn_translate')} (Import)`"><Icon name="download" /></button>
                              <button class="icon-text-btn" @click="toggleHideAudio(aud.rel_path)" :title="hiddenAudio.includes(aud.rel_path) ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenAudio.includes(aud.rel_path) ? 'eye-off' : 'eye'" /></button>
                              <button v-if="aud.is_translated" class="icon-text-btn" style="color: var(--error-text);" @click="revertAudio(aud)" :title="t('revert_audio')"><Icon name="undo" /></button>
                              <button class="icon-text-btn" @click="openImgFolder(aud.original_path)" :title="t('open_folder')"><Icon name="folder" /></button>
                          </div>
                      </div>
                  </div>
              </div>

              <div v-if="filteredAudio.length === 0" style="text-align: center; color: var(--text-muted); padding: 40px;">{{ t('no_audio_found') }}</div>

              <div v-if="audioTotalPages > 1" class="pagination-container">
                  <button class="btn btn-secondary" :disabled="audioCurrentPage === 1" @click="audioCurrentPage = 1">«</button>
                  <button class="btn btn-secondary" :disabled="audioCurrentPage === 1" @click="audioCurrentPage--">‹</button>
                  <span class="pagination-info">{{ t('page') }} <input type="number" v-model.lazy="audioCurrentPage" min="1" :max="audioTotalPages" class="page-input" @change="validateAudioPage" /> {{ t('out_of') }} {{ audioTotalPages }}</span>
                  <button class="btn btn-secondary" :disabled="audioCurrentPage === audioTotalPages" @click="audioCurrentPage++">›</button>
                  <button class="btn btn-secondary" :disabled="audioCurrentPage === audioTotalPages" @click="audioCurrentPage = audioTotalPages">»</button>
              </div>
          </div>
      </main>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { t } from '../locales.js';
import Icon from './Icon.vue';
import { projectPath, targetLang, hiddenAudio, hiddenFolders, showHiddenMedia, showMsg, getFolderFromPath } from '../store.js';

const audioFiles = ref([]);
const audioSearch = ref('');
const audioSelectedFolder = ref('');
const audioCurrentPage = ref(1);
const audioItemsPerPage = 100;
const isAudioLoading = ref(false);

// --- Drag&drop аудиофайлов из ОС ---
const dragOverPath = ref(null);
const AUDIO_EXTS = ['ogg', 'mp3', 'wav'];
let unlistenDrop = null;

function cardPathAtPoint(pos) {
    if (!pos) return null;
    const dpr = window.devicePixelRatio || 1;
    const el = document.elementFromPoint(pos.x / dpr, pos.y / dpr);
    const card = el && el.closest ? el.closest('.gallery-card') : null;
    return card ? card.getAttribute('data-relpath') : null;
}

async function importDroppedAudio(relPath, sourceFilePath) {
    const aud = audioFiles.value.find(a => a.rel_path === relPath);
    if (!aud) return;
    const ext = (sourceFilePath.split('.').pop() || '').toLowerCase();
    if (!AUDIO_EXTS.includes(ext)) {
        showMsg('error', t('drop_not_audio'));
        return;
    }
    try {
        const translated_path = await invoke('import_localized_audio', {
            projectPath: projectPath.value, targetLang: targetLang.value, relPath, sourceFilePath
        });
        aud.is_translated = true; aud.translated_path = translated_path;
        showMsg('success', t('audio_copied'));
    } catch (e) { showMsg('error', e.toString()); }
}

watch([audioSearch, audioSelectedFolder], () => { audioCurrentPage.value = 1; });
// Перезагружаем список аудио при смене проекта/языка, даже если вкладка не переоткрывалась.
watch([projectPath, targetLang], () => { loadAudio(); });

onMounted(() => {
    loadAudio();
    getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'drop') {
            const relPath = cardPathAtPoint(p.position);
            dragOverPath.value = null;
            if (relPath && p.paths && p.paths.length) importDroppedAudio(relPath, p.paths[0]);
        } else if (p.type === 'leave' || p.type === 'cancelled') {
            dragOverPath.value = null;
        } else {
            dragOverPath.value = cardPathAtPoint(p.position);
        }
    }).then(u => { unlistenDrop = u; }).catch(() => {});
});
onUnmounted(() => { if (unlistenDrop) { unlistenDrop(); unlistenDrop = null; } });

const audioFolders = computed(() => {
    const folders = new Set();
    audioFiles.value.forEach(aud => { folders.add(getFolderFromPath(aud.rel_path)); });
    return Array.from(folders).sort().filter(f => f !== '/'); 
});

async function loadAudio() {
    if (!projectPath.value) return;
    isAudioLoading.value = true;
    try {
        audioFiles.value = await invoke('get_audio_list', { projectPath: projectPath.value, targetLang: targetLang.value });
    } catch(e) { showMsg('error', e.toString()); } 
    finally { isAudioLoading.value = false; }
}

const filteredAudio = computed(() => {
    let result = audioFiles.value;
    if (!showHiddenMedia.value) {
        result = result.filter(aud => !hiddenAudio.value.includes(aud.rel_path) && !hiddenFolders.value.includes(getFolderFromPath(aud.rel_path)));
    }
    if (audioSelectedFolder.value) {
        result = audioSelectedFolder.value === '/' ? result.filter(aud => !aud.rel_path.includes('/')) : result.filter(aud => getFolderFromPath(aud.rel_path) === audioSelectedFolder.value);
    }
    if (audioSearch.value) {
        const q = audioSearch.value.toLowerCase();
        result = result.filter(a =>
            a.rel_path.toLowerCase().includes(q) ||
            (a.mapped_text && a.mapped_text.toLowerCase().includes(q))
        );
    }
    return result;
});

const audioTotalPages = computed(() => Math.ceil(filteredAudio.value.length / audioItemsPerPage) || 1);
const paginatedAudio = computed(() => {
    const start = (audioCurrentPage.value - 1) * audioItemsPerPage;
    return filteredAudio.value.slice(start, start + audioItemsPerPage);
});

function validateAudioPage() {
    let p = parseInt(audioCurrentPage.value);
    if (isNaN(p) || p < 1) p = 1;
    if (p > audioTotalPages.value) p = audioTotalPages.value;
    audioCurrentPage.value = p;
}

function getAudioSrc(aud) {
    const path = aud.is_translated && aud.translated_path ? aud.translated_path : aud.original_path;
    return convertFileSrc(path);
}

// Одновременно играет только один плеер: при старте паузим все остальные
function onAudioPlay(e) {
    document.querySelectorAll('audio').forEach(a => { if (a !== e.target) a.pause(); });
}

function toggleHideFolder(folder) {
  if (hiddenFolders.value.includes(folder)) hiddenFolders.value = hiddenFolders.value.filter(f => f !== folder);
  else hiddenFolders.value.push(folder);
}

function toggleHideAudio(relPath) {
  if (hiddenAudio.value.includes(relPath)) hiddenAudio.value = hiddenAudio.value.filter(p => p !== relPath);
  else hiddenAudio.value.push(relPath);
}

async function importAudioDialog(aud) {
    try {
        const selected = await openDialog({ multiple: false, filters:[{ name: 'Audio', extensions:['ogg', 'mp3', 'wav'] }] });
        if (selected) {
            const translated_path = await invoke('import_localized_audio', {
                projectPath: projectPath.value, targetLang: targetLang.value, relPath: aud.rel_path, sourceFilePath: selected
            });
            aud.is_translated = true; aud.translated_path = translated_path;
            showMsg('success', t('audio_copied'));
        }
    } catch(e) { showMsg('error', e.toString()); }
}

async function revertAudio(aud) {
    if (!confirm(t('confirm_audio_revert'))) return;
    try {
        await invoke('delete_localized_audio', { projectPath: projectPath.value, targetLang: targetLang.value, relPath: aud.rel_path });
        aud.is_translated = false; aud.translated_path = null;
        showMsg('success', t('audio_reverted'));
    } catch(e) { showMsg('error', e.toString()); }
}

async function openImgFolder(path) {
    try { await invoke('open_in_explorer', { path }); } catch(e) { showMsg('error', e.toString()); }
}
</script>