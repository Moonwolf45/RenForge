<template>
  <div class="gallery-workspace">
      <aside class="sidebar media-sidebar">
          <div class="sidebar-title">{{ t('folders') }}</div>
          <div class="sidebar-list">
              <div class="sidebar-item media-folder-item" :class="{ active: gallerySelectedFolder === '' }" @click="gallerySelectedFolder = ''">
                  <span class="folder-name">{{ t('all_folders') }}</span>
              </div>
              <div class="sidebar-item media-folder-item" :class="{ active: gallerySelectedFolder === '/', 'is-faded': hiddenFolders.includes('/') }" @click="gallerySelectedFolder = '/'">
                  <span class="folder-name">{{ t('root_folder') }}</span>
                  <button class="icon-text-btn folder-eye-btn" @click.stop="toggleHideFolder('/')" :title="hiddenFolders.includes('/') ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenFolders.includes('/') ? 'eye-off' : 'eye'" /></button>
              </div>
              <div class="sidebar-item media-folder-item" v-for="f in galleryFolders" :key="f" :class="{ active: gallerySelectedFolder === f, 'is-faded': hiddenFolders.includes(f) }" @click="gallerySelectedFolder = f">
                  <span class="folder-name" :title="f">{{ f }}</span>
                  <button class="icon-text-btn folder-eye-btn" @click.stop="toggleHideFolder(f)" :title="hiddenFolders.includes(f) ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenFolders.includes(f) ? 'eye-off' : 'eye'" /></button>
              </div>
          </div>
      </aside>

      <main class="media-main">
          <div class="gallery-header">
              <h2>{{ t('images') }} <span style="color: var(--text-muted); font-weight: normal; font-size: 14px; margin-left: 5px;">› {{ gallerySelectedFolder === '' ? t('all_folders') : gallerySelectedFolder }}</span></h2>
              <div class="gallery-actions" style="display: flex; gap: 15px;">
                  <label class="toggle-hidden" style="margin: 0; align-items: center; display: flex;" v-if="hiddenImages.length > 0 || hiddenFolders.length > 0">
                    <input type="checkbox" v-model="showHiddenMedia">
                    {{ t('show_hidden') }}
                  </label>
                  <input type="text" v-model="gallerySearch" :placeholder="t('search_placeholder')" class="search-input" style="width: 250px; padding: 8px 15px;"/>
              </div>
          </div>

          <div v-if="isGalleryLoading" class="gallery-loading" style="padding: 30px; text-align: center;">
              <p>{{ t('loading_gallery') }}</p>
          </div>

          <div v-else class="gallery-scroll-container">
              <div class="gallery-grid">
                  <div class="gallery-card" v-for="img in paginatedGallery" :key="img.rel_path" :data-relpath="img.rel_path" :class="{ 'is-hidden': hiddenImages.includes(img.rel_path), 'drag-over': dragOverPath === img.rel_path }">
                      <div class="gallery-img-container" @click="importImageDialog(img)" :title="t('drop_here')">
                          <img :src="getImgSrc(img)" loading="lazy" class="gallery-img" />
                          <div class="gallery-img-overlay"><span>{{ t('drop_here') }}</span></div>
                          <button class="gallery-zoom-btn" @click.stop="openLightbox(img)" :title="t('enlarge')" aria-label="enlarge">
                              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                  <path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M3 16v3a2 2 0 0 0 2 2h3"/>
                              </svg>
                          </button>
                          <div v-if="img.is_translated" class="status-badge status-done img-badge">{{ t('status_translated') }}</div>
                      </div>
                      <div class="gallery-card-info">
                          <div class="img-path" :title="img.rel_path">{{ img.rel_path }}</div>
                          <div class="card-actions">
                              <button class="icon-text-btn" @click="openImgFolder(img.original_path)" :title="t('open_folder')"><Icon name="folder" /></button>
                              <button class="icon-text-btn" @click="toggleHideImage(img.rel_path)" :title="hiddenImages.includes(img.rel_path) ? t('btn_show') : t('btn_hide')"><Icon :name="hiddenImages.includes(img.rel_path) ? 'eye-off' : 'eye'" /></button>
                              <button v-if="img.is_translated" class="icon-text-btn" style="color: var(--error-text);" @click="revertImage(img)" :title="t('revert_img')"><Icon name="undo" /></button>
                          </div>
                      </div>
                  </div>
              </div>

              <div v-if="filteredGallery.length === 0" style="text-align: center; color: var(--text-muted); padding: 40px;">{{ t('no_images_found') }}</div>

              <div v-if="galleryTotalPages > 1" class="pagination-container">
                  <button class="btn btn-secondary" :disabled="galleryCurrentPage === 1" @click="galleryCurrentPage = 1">«</button>
                  <button class="btn btn-secondary" :disabled="galleryCurrentPage === 1" @click="galleryCurrentPage--">‹</button>
                  <span class="pagination-info">{{ t('page') }} <input type="number" v-model.lazy="galleryCurrentPage" min="1" :max="galleryTotalPages" class="page-input" @change="validateGalleryPage" /> {{ t('out_of') }} {{ galleryTotalPages }}</span>
                  <button class="btn btn-secondary" :disabled="galleryCurrentPage === galleryTotalPages" @click="galleryCurrentPage++">›</button>
                  <button class="btn btn-secondary" :disabled="galleryCurrentPage === galleryTotalPages" @click="galleryCurrentPage = galleryTotalPages">»</button>
              </div>
          </div>
      </main>

      <!-- Лайтбокс: окошко с зумом колёсиком и перетаскиванием -->
      <div v-if="lightboxImg" class="lightbox-backdrop" @click="closeLightbox">
          <div class="lightbox-content" @click.stop>
              <button class="lightbox-close" @click="closeLightbox" :title="t('close')">✕</button>
              <button v-if="filteredGallery.length > 1" class="lightbox-nav lightbox-prev" @click="lightboxPrev" :title="t('img_prev')">‹</button>
              <button v-if="filteredGallery.length > 1" class="lightbox-nav lightbox-next" @click="lightboxNext" :title="t('img_next')">›</button>
              <div
                  ref="viewportRef"
                  class="lightbox-viewport"
                  :class="[{ grabbing: isDragging }, 'bg-' + lightboxBg]"
                  @wheel="onWheel"
                  @mousedown="onDragStart"
              >
                  <img
                      :src="lightboxSrc"
                      class="lightbox-img"
                      draggable="false"
                      :style="{ width: imgDisplayW + 'px', height: imgDisplayH + 'px' }"
                      @load="onImgLoad"
                  />
              </div>
              <div class="lightbox-bar">
                  <div class="lightbox-zoom-ctrl">
                      <button class="lb-zoom-btn" @click="setZoom(zoom / 1.25)" :title="t('zoom_out')">−</button>
                      <input type="range" class="lb-zoom-slider" :min="minZoom" :max="maxZoom" step="0.02" :value="zoom" @input="onSlider" />
                      <button class="lb-zoom-btn" @click="setZoom(zoom * 1.25)" :title="t('zoom_in')">+</button>
                      <span class="lb-zoom-pct">{{ Math.round(zoom * 100) }}%</span>
                      <button class="lb-zoom-btn lb-fit" @click="fitToViewport" :title="t('fit')">⤢</button>
                  </div>
                  <div class="lightbox-bg-ctrl" :title="t('bg_label')">
                      <button v-for="m in ['checker','dark','light']" :key="m"
                              class="lb-bg-btn" :class="['sw-' + m, { active: lightboxBg === m }]"
                              @click="lightboxBg = m" :title="t('bg_' + m)" :aria-label="t('bg_' + m)"></button>
                  </div>
                  <div v-if="lightboxImg.is_translated" class="lightbox-toggle">
                      <button :class="{ active: lightboxShowOriginal }" @click="lightboxShowOriginal = true">{{ t('view_original') }}</button>
                      <button :class="{ active: !lightboxShowOriginal }" @click="lightboxShowOriginal = false">{{ t('view_localized') }}</button>
                  </div>
              </div>
              <div class="lightbox-path" :title="lightboxImg.rel_path"><span v-if="filteredGallery.length > 1" class="lightbox-counter">{{ lightboxIndex + 1 }} / {{ filteredGallery.length }}</span>{{ lightboxImg.rel_path }}</div>
          </div>
      </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { t } from '../locales.js';
import Icon from './Icon.vue';
import { projectPath, targetLang, hiddenImages, hiddenFolders, showHiddenMedia, showMsg, getFolderFromPath } from '../store.js';

const galleryImages = ref([]);
const gallerySearch = ref('');
const gallerySelectedFolder = ref('');
const galleryCurrentPage = ref(1);
const galleryItemsPerPage = 100;
const isGalleryLoading = ref(false);

// Подложка лайтбокса для оценки прозрачности PNG: 'checker' (мягкая тематическая шахматка)
// | 'dark' | 'light'. Хранится в localStorage.
const lightboxBg = ref(localStorage.getItem('renforge_lightbox_bg') || 'checker');
watch(lightboxBg, (v) => localStorage.setItem('renforge_lightbox_bg', v));

// --- Drag&drop файлов с рабочего стола/папки ---
const dragOverPath = ref(null);
const IMG_EXTS = ['png', 'jpg', 'jpeg', 'webp'];
let unlistenDrop = null;

// Карточка под курсором по физической позиции из Tauri (переводим в CSS-пиксели)
function cardPathAtPoint(pos) {
    if (!pos) return null;
    const dpr = window.devicePixelRatio || 1;
    const el = document.elementFromPoint(pos.x / dpr, pos.y / dpr);
    const card = el && el.closest ? el.closest('.gallery-card') : null;
    return card ? card.getAttribute('data-relpath') : null;
}

async function importDroppedImage(relPath, sourceFilePath) {
    const img = galleryImages.value.find(i => i.rel_path === relPath);
    if (!img) return;
    const ext = (sourceFilePath.split('.').pop() || '').toLowerCase();
    if (!IMG_EXTS.includes(ext)) {
        showMsg('error', t('drop_not_image'));
        return;
    }
    try {
        const translated_path = await invoke('import_localized_image', {
            projectPath: projectPath.value, targetLang: targetLang.value, relPath, sourceFilePath
        });
        img.is_translated = true; img.translated_path = translated_path;
        showMsg('success', t('img_copied'));
    } catch (e) { showMsg('error', e.toString()); }
}

// --- Лайтбокс с зумом и панорамой ---
const lightboxImg = ref(null);
const lightboxShowOriginal = ref(false);
const viewportRef = ref(null);
const zoom = ref(1);
const minZoom = 0.05;
const maxZoom = 8;
const naturalW = ref(0);
const naturalH = ref(0);
const isDragging = ref(false);
let fitOnLoad = false;
let dragSX = 0, dragSY = 0, dragSL = 0, dragST = 0;

const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

const lightboxSrc = computed(() => {
    const img = lightboxImg.value;
    if (!img) return '';
    const useTranslated = !lightboxShowOriginal.value && img.is_translated && img.translated_path;
    return convertFileSrc(useTranslated ? img.translated_path : img.original_path);
});

const imgDisplayW = computed(() => Math.round(naturalW.value * zoom.value));
const imgDisplayH = computed(() => Math.round(naturalH.value * zoom.value));

function openLightbox(img) {
    const idx = filteredGallery.value.findIndex(i => i.rel_path === img.rel_path);
    showLightboxIndex(idx < 0 ? 0 : idx);
}
// Показать изображение по индексу в текущем отфильтрованном списке (с заворотом).
// Предзагружаем картинку и только потом меняем src — без мерцания при листании.
let navToken = 0;
function srcForImage(img) {
    const useTranslated = img.is_translated && img.translated_path;
    return convertFileSrc(useTranslated ? img.translated_path : img.original_path);
}
function showLightboxIndex(idx) {
    const list = filteredGallery.value;
    if (!list.length) return;
    const i = ((idx % list.length) + list.length) % list.length;
    const img = list[i];
    const token = ++navToken;
    const apply = () => {
        if (token !== navToken) return; // пришёл более свежий запрос — игнорируем
        lightboxShowOriginal.value = false;
        fitOnLoad = true;
        lightboxImg.value = img; // дименшены НЕ обнуляем — старая картинка держится до onImgLoad
    };
    const pre = new Image();
    pre.onload = apply;
    pre.onerror = apply;
    pre.src = srcForImage(img);
}
const lightboxIndex = computed(() => {
    if (!lightboxImg.value) return -1;
    return filteredGallery.value.findIndex(i => i.rel_path === lightboxImg.value.rel_path);
});
function lightboxPrev() { showLightboxIndex(lightboxIndex.value - 1); }
function lightboxNext() { showLightboxIndex(lightboxIndex.value + 1); }
function closeLightbox() { lightboxImg.value = null; }
function onLightboxKey(e) {
    if (!lightboxImg.value) return;
    if (e.key === 'Escape') closeLightbox();
    else if (e.key === 'ArrowLeft') lightboxPrev();
    else if (e.key === 'ArrowRight') lightboxNext();
}

function onImgLoad(e) {
    naturalW.value = e.target.naturalWidth || 1;
    naturalH.value = e.target.naturalHeight || 1;
    if (fitOnLoad) {
        fitOnLoad = false;
        // Ждём раскладку окошка, потом считаем реальный fit
        nextTick(() => {
            const vp = viewportRef.value;
            const fit = (vp && vp.clientWidth > 0)
                ? Math.min(vp.clientWidth / naturalW.value, vp.clientHeight / naturalH.value)
                : 1;
            // всегда максимальный масштаб, при котором картинка целиком влезает в окно
            zoom.value = clamp(fit, minZoom, maxZoom);
            nextTick(centerViewport);
        });
    } else {
        nextTick(centerViewport);
    }
}

function centerViewport() {
    const vp = viewportRef.value;
    if (!vp) return;
    vp.scrollLeft = (vp.scrollWidth - vp.clientWidth) / 2;
    vp.scrollTop = (vp.scrollHeight - vp.clientHeight) / 2;
}

// Зум вокруг точки (ax, ay) в координатах окошка
function zoomAround(newZoom, ax, ay) {
    const vp = viewportRef.value;
    const nz = clamp(newZoom, minZoom, maxZoom);
    const old = zoom.value;
    if (!vp || nz === old) { zoom.value = nz; return; }
    const ratio = nz / old;
    const sl = vp.scrollLeft, st = vp.scrollTop;
    zoom.value = nz;
    nextTick(() => {
        vp.scrollLeft = (sl + ax) * ratio - ax;
        vp.scrollTop = (st + ay) * ratio - ay;
    });
}
function setZoom(z) {
    const vp = viewportRef.value;
    zoomAround(z, vp ? vp.clientWidth / 2 : 0, vp ? vp.clientHeight / 2 : 0);
}
function onSlider(e) { setZoom(parseFloat(e.target.value)); }

function onWheel(e) {
    e.preventDefault();
    const vp = viewportRef.value;
    if (!vp) return;
    const rect = vp.getBoundingClientRect();
    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
    zoomAround(zoom.value * factor, e.clientX - rect.left, e.clientY - rect.top);
}

function fitToViewport() {
    const vp = viewportRef.value;
    if (!vp || !naturalW.value) return;
    const z = Math.min(vp.clientWidth / naturalW.value, vp.clientHeight / naturalH.value);
    zoom.value = clamp(z, minZoom, maxZoom);
    nextTick(centerViewport);
}

function onDragStart(e) {
    const vp = viewportRef.value;
    if (!vp) return;
    isDragging.value = true;
    dragSX = e.clientX; dragSY = e.clientY;
    dragSL = vp.scrollLeft; dragST = vp.scrollTop;
    e.preventDefault();
}
function onDragMove(e) {
    if (!isDragging.value) return;
    const vp = viewportRef.value;
    if (!vp) return;
    vp.scrollLeft = dragSL - (e.clientX - dragSX);
    vp.scrollTop = dragST - (e.clientY - dragSY);
}
function onDragEnd() { isDragging.value = false; }

watch([gallerySearch, gallerySelectedFolder], () => { galleryCurrentPage.value = 1; });
// Перезагружаем галерею при смене проекта/языка, даже если вкладка не переоткрывалась.
watch([projectPath, targetLang], () => { loadGallery(); });

onMounted(() => {
    loadGallery();
    window.addEventListener('keydown', onLightboxKey);
    window.addEventListener('mousemove', onDragMove);
    window.addEventListener('mouseup', onDragEnd);
    // Нативный drag&drop файлов из ОС (Tauri отдаёт реальные пути + позицию курсора)
    getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'drop') {
            const relPath = cardPathAtPoint(p.position);
            dragOverPath.value = null;
            if (relPath && p.paths && p.paths.length) {
                importDroppedImage(relPath, p.paths[0]);
            }
        } else if (p.type === 'leave' || p.type === 'cancelled') {
            dragOverPath.value = null;
        } else {
            // enter / over — подсвечиваем карточку под курсором
            dragOverPath.value = cardPathAtPoint(p.position);
        }
    }).then(u => { unlistenDrop = u; }).catch(() => {});
});
onUnmounted(() => {
    window.removeEventListener('keydown', onLightboxKey);
    window.removeEventListener('mousemove', onDragMove);
    window.removeEventListener('mouseup', onDragEnd);
    if (unlistenDrop) { unlistenDrop(); unlistenDrop = null; }
});

const galleryFolders = computed(() => {
    const folders = new Set();
    galleryImages.value.forEach(img => { folders.add(getFolderFromPath(img.rel_path)); });
    return Array.from(folders).sort().filter(f => f !== '/'); 
});

async function loadGallery() {
    if (!projectPath.value) return;
    isGalleryLoading.value = true;
    try {
        galleryImages.value = await invoke('get_images_list', { projectPath: projectPath.value, targetLang: targetLang.value });
    } catch(e) { showMsg('error', e.toString()); } 
    finally { isGalleryLoading.value = false; }
}

const filteredGallery = computed(() => {
    let result = galleryImages.value;
    if (!showHiddenMedia.value) {
        result = result.filter(img => !hiddenImages.value.includes(img.rel_path) && !hiddenFolders.value.includes(getFolderFromPath(img.rel_path)));
    }
    if (gallerySelectedFolder.value) {
        result = gallerySelectedFolder.value === '/' ? result.filter(img => !img.rel_path.includes('/')) : result.filter(img => getFolderFromPath(img.rel_path) === gallerySelectedFolder.value);
    }
    if (gallerySearch.value) result = result.filter(img => img.rel_path.toLowerCase().includes(gallerySearch.value.toLowerCase()));
    return result;
});

const galleryTotalPages = computed(() => Math.ceil(filteredGallery.value.length / galleryItemsPerPage) || 1);
const paginatedGallery = computed(() => {
    const start = (galleryCurrentPage.value - 1) * galleryItemsPerPage;
    return filteredGallery.value.slice(start, start + galleryItemsPerPage);
});

function validateGalleryPage() {
    let p = parseInt(galleryCurrentPage.value);
    if (isNaN(p) || p < 1) p = 1;
    if (p > galleryTotalPages.value) p = galleryTotalPages.value;
    galleryCurrentPage.value = p;
}

function getImgSrc(img) {
    const path = img.is_translated && img.translated_path ? img.translated_path : img.original_path;
    return convertFileSrc(path);
}

function toggleHideFolder(folder) {
  if (hiddenFolders.value.includes(folder)) hiddenFolders.value = hiddenFolders.value.filter(f => f !== folder);
  else hiddenFolders.value.push(folder);
}

function toggleHideImage(relPath) {
  if (hiddenImages.value.includes(relPath)) hiddenImages.value = hiddenImages.value.filter(p => p !== relPath);
  else hiddenImages.value.push(relPath);
}

async function importImageDialog(img) {
    try {
        const selected = await openDialog({ multiple: false, filters:[{ name: 'Images', extensions:['png', 'jpg', 'jpeg', 'webp'] }] });
        if (selected) {
            const translated_path = await invoke('import_localized_image', {
                projectPath: projectPath.value, targetLang: targetLang.value, relPath: img.rel_path, sourceFilePath: selected
            });
            img.is_translated = true; img.translated_path = translated_path;
            showMsg('success', t('img_copied'));
        }
    } catch(e) { showMsg('error', e.toString()); }
}

async function revertImage(img) {
    if (!confirm(t('confirm_revert'))) return;
    try {
        await invoke('delete_localized_image', { projectPath: projectPath.value, targetLang: targetLang.value, relPath: img.rel_path });
        img.is_translated = false; img.translated_path = null;
        showMsg('success', t('img_reverted'));
    } catch(e) { showMsg('error', e.toString()); }
}

async function openImgFolder(path) {
    try { await invoke('open_in_explorer', { path }); } catch(e) { showMsg('error', e.toString()); }
}
</script>

<style>
/* Кнопка увеличения на карточке */
.gallery-img-container { position: relative; }
.gallery-zoom-btn {
    position: absolute;
    top: 6px;
    left: 6px;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.5);
    color: #fff;
    padding: 5px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s ease, background 0.15s ease, border-color 0.15s ease, transform 0.1s ease;
}
.gallery-img-container:hover .gallery-zoom-btn { opacity: 1; }
.gallery-zoom-btn:hover {
    background: var(--accent, #4a9eff);
    border-color: var(--accent, #4a9eff);
    transform: scale(1.08);
}
.gallery-zoom-btn svg { display: block; }

/* Лайтбокс */
.lightbox-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: rgba(0, 0, 0, 0.82);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
    backdrop-filter: blur(2px);
}
.lightbox-content {
    display: flex;
    flex-direction: column;
    width: 90vw;
    height: 88vh;
    max-width: 1100px;
    max-height: 860px;
    background: var(--bg-base, #1e1e1e);
    border: 1px solid var(--border-main, #444);
    border-radius: 10px;
    overflow: hidden;
    position: relative;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
}
.lightbox-close {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 2;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    font-size: 16px;
    cursor: pointer;
}
.lightbox-close:hover { background: rgba(0, 0, 0, 0.9); }
.lightbox-nav {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    z-index: 2;
    width: 44px;
    height: 64px;
    border: none;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
    font-size: 32px;
    line-height: 1;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s ease;
}
.lightbox-nav:hover { background: var(--accent, #4a9eff); }
.lightbox-prev { left: 0; border-radius: 0 8px 8px 0; }
.lightbox-next { right: 0; border-radius: 8px 0 0 8px; }
.lightbox-counter { color: var(--accent, #4a9eff); font-weight: 700; margin-right: 10px; }
.lightbox-viewport {
    flex: 1;
    min-height: 0;
    width: 100%;
    overflow: auto;
    cursor: grab;
    display: grid;
    align-content: safe center;
    justify-content: safe center;
}
/* Подложки для оценки прозрачности PNG (переключаются). */
/* Мягкая тематическая шахматка: низкоконтрастная, адаптируется к теме (текст на базе). */
.lightbox-viewport.bg-checker {
    background-color: var(--bg-base, #1e1e1e);
    background-image:
        linear-gradient(45deg, color-mix(in srgb, var(--text-main) 6%, transparent) 25%, transparent 25%),
        linear-gradient(-45deg, color-mix(in srgb, var(--text-main) 6%, transparent) 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, color-mix(in srgb, var(--text-main) 6%, transparent) 75%),
        linear-gradient(-45deg, transparent 75%, color-mix(in srgb, var(--text-main) 6%, transparent) 75%);
    background-size: 24px 24px;
    background-position: 0 0, 0 12px, 12px -12px, -12px 0;
}
.lightbox-viewport.bg-dark { background-color: #171717; }
.lightbox-viewport.bg-light { background-color: #dcdcdc; }

/* Переключатель подложки в панели лайтбокса */
.lightbox-bg-ctrl { display: flex; align-items: center; gap: 6px; }
.lb-bg-btn {
    width: 22px; height: 22px; padding: 0; border-radius: 5px; cursor: pointer;
    border: 1px solid var(--border-input); background-clip: padding-box;
    transition: transform 0.1s ease, border-color 0.15s ease, box-shadow 0.15s ease;
}
.lb-bg-btn:hover { transform: translateY(-1px); }
.lb-bg-btn.active { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent); }
.lb-bg-btn.sw-dark { background: #171717; }
.lb-bg-btn.sw-light { background: #dcdcdc; }
.lb-bg-btn.sw-checker {
    background-color: #171717;
    background-image:
        linear-gradient(45deg, #565656 25%, transparent 25%),
        linear-gradient(-45deg, #565656 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, #565656 75%),
        linear-gradient(-45deg, transparent 75%, #565656 75%);
    background-size: 10px 10px;
    background-position: 0 0, 0 5px, 5px -5px, -5px 0;
}
.lightbox-viewport.grabbing { cursor: grabbing; }
.lightbox-img {
    display: block;
    max-width: none;
    user-select: none;
    -webkit-user-drag: none;
    image-rendering: auto;
}
/* видимые полосы прокрутки (ползунки) */
.lightbox-viewport::-webkit-scrollbar { width: 12px; height: 12px; }
.lightbox-viewport::-webkit-scrollbar-thumb {
    background: var(--accent, #4a9eff);
    border-radius: 6px;
    border: 2px solid rgba(0, 0, 0, 0.3);
}
.lightbox-viewport::-webkit-scrollbar-track { background: rgba(0, 0, 0, 0.3); }

.lightbox-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 8px 14px;
    border-top: 1px solid var(--border-main, #444);
    background: var(--bg-base, #1e1e1e);
}
.lightbox-zoom-ctrl { display: flex; align-items: center; gap: 8px; flex: 1; }
.lb-zoom-btn {
    width: 26px;
    height: 26px;
    border: 1px solid var(--border-main, #444);
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted, #aaa);
    font-size: 15px;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
}
.lb-zoom-btn:hover { background: var(--accent, #4a9eff); color: var(--accent-contrast, #fff); border-color: var(--accent, #4a9eff); }
.lb-zoom-slider { flex: 1; max-width: 240px; accent-color: var(--accent, #4a9eff); cursor: pointer; }
.lb-zoom-pct { font-size: 12px; color: var(--text-muted, #aaa); min-width: 44px; text-align: right; }
.lb-fit { font-size: 14px; }

.lightbox-path {
    font-size: 12px;
    color: var(--text-muted, #aaa);
    padding: 6px 14px 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: var(--bg-base, #1e1e1e);
}
.lightbox-toggle { display: flex; gap: 0; flex-shrink: 0; }
.lightbox-toggle button {
    border: 1px solid var(--border-main, #444);
    background: transparent;
    color: var(--text-muted, #aaa);
    font-size: 12px;
    padding: 5px 12px;
    cursor: pointer;
}
.lightbox-toggle button:first-child { border-radius: 6px 0 0 6px; border-right: none; }
.lightbox-toggle button:last-child { border-radius: 0 6px 6px 0; }
.lightbox-toggle button.active {
    background: var(--accent, #4a9eff);
    color: var(--accent-contrast, #fff);
    border-color: var(--accent, #4a9eff);
}
</style>
