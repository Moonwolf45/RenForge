<template>
  <div class="app-container" :data-theme="uiTheme" :style="accentStyle">
    <Header />
    <div v-if="activePopover" class="dropdown-overlay-bg" @click="activePopover = null"></div>
    <GlobalMessages />

    <Dashboard v-if="currentMode === 'dashboard'" />
    <Editor v-if="currentMode === 'editor'" />
    <ImageGallery v-if="currentMode === 'gallery'" />
    <AudioGallery v-if="currentMode === 'audio'" />

    <AiModal v-if="isAiModalOpen" />
    <UpdateModal v-if="showUpdateModal" />
    <TmModal v-if="showTmModal" />
    <SourceViewer v-if="showSourceModal" />
    <AddStringModal v-if="showAddStringModal" />
    <DeliveryHooksModal v-if="showDeliveryHooksModal" />
    <AboutModal v-if="showAboutModal" />
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, computed } from 'vue';
import { uiTheme, currentMode, activePopover, isAiModalOpen, showUpdateModal, showTmModal, showSourceModal, showAddStringModal, showDeliveryHooksModal, showAboutModal, uiAccent, resolveAccent, contrastFor } from './store.js';

import Header from './components/Header.vue';
import GlobalMessages from './components/GlobalMessages.vue';
import AiModal from './components/AiModal.vue';
import UpdateModal from './components/UpdateModal.vue';
import TmModal from './components/TmModal.vue';
import SourceViewer from './components/SourceViewer.vue';
import AddStringModal from './components/AddStringModal.vue';
import DeliveryHooksModal from './components/DeliveryHooksModal.vue';
import AboutModal from './components/AboutModal.vue';
import Dashboard from './components/Dashboard.vue';
import Editor from './components/Editor.vue';
import ImageGallery from './components/ImageGallery.vue';
import AudioGallery from './components/AudioGallery.vue';

const accentStyle = computed(() => {
  const a = resolveAccent(uiAccent.value);
  return { '--accent': a.c, '--accent-hover': a.h, '--accent-contrast': contrastFor(a.c) };
});

function handleContextMenu(e) {
  const target = e.target;
  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable || target.closest('.raw-code') !== null;
  const hasSelection = window.getSelection().toString().length > 0;
  if (!isInput && !hasSelection) e.preventDefault();
}

onMounted(() => { window.addEventListener('contextmenu', handleContextMenu); });
onUnmounted(() => { window.removeEventListener('contextmenu', handleContextMenu); });
</script>

<style>
/* Здесь подключаем стили из 4 пункта: */
@import './assets/style.css';
</style>