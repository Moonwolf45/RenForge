<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content update-modal">
      <div class="modal-header">
        <h2>{{ t('update_title') }}</h2>
        <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
      </div>

      <div class="modal-scroll-body">
        <p class="update-intro">{{ t('update_intro') }}</p>

        <div class="update-pick">
          <div class="update-pick-label">{{ t('update_old_folder') }}</div>
          <div class="update-pick-row">
            <span class="update-path" :title="oldPath">{{ oldPath || t('update_not_selected') }}</span>
            <button class="btn btn-secondary" @click="pickFolder"><Icon name="folder" :size="15" /> {{ t('select_folder') }}</button>
          </div>
        </div>

        <!-- ОТЧЁТ -->
        <div v-if="report" class="update-report">
          <div class="ur-card ur-exact">
            <div class="ur-num">{{ report.carried_exact }}</div>
            <div class="ur-label">{{ t('update_exact') }}</div>
          </div>
          <div class="ur-card ur-fuzzy">
            <div class="ur-num">{{ report.carried_fuzzy }}</div>
            <div class="ur-label">{{ t('update_fuzzy') }}</div>
          </div>
          <div class="ur-card ur-new">
            <div class="ur-num">{{ report.new_strings }}</div>
            <div class="ur-label">{{ t('update_new') }}</div>
          </div>
          <div class="ur-card ur-gone">
            <div class="ur-num">{{ report.old_unused }}</div>
            <div class="ur-label">{{ t('update_gone') }}</div>
          </div>
        </div>
        <p v-if="report" class="update-untr">{{ t('update_still') }}: <b>{{ report.still_untranslated }}</b></p>
        <p v-if="report && report.carried_fuzzy > 0" class="update-hint">{{ t('update_review_hint') }}</p>
      </div>

      <div class="ai-footer">
        <span class="ai-footer-info">{{ t('update_safe_note') }}</span>
        <button class="btn btn-primary ai-run-btn" @click="run" :disabled="!oldPath || isRunning || !projectPath">
          <span v-if="isRunning" class="btn-spinner"></span>
          {{ isRunning ? t('update_running') : (report ? t('update_done_btn') : t('update_run_btn')) }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { showUpdateModal, projectPath, showMsg } from '../store.js';
import { migrateTranslations } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const oldPath = ref('');
const report = ref(null);
const isRunning = ref(false);

function close() { showUpdateModal.value = false; }

async function pickFolder() {
  try {
    const selected = await openDialog({ multiple: false, directory: true });
    if (selected) { oldPath.value = selected; report.value = null; }
  } catch (e) { showMsg('error', e.toString()); }
}

async function run() {
  if (!oldPath.value || !projectPath.value) return;
  isRunning.value = true;
  try {
    report.value = await migrateTranslations(oldPath.value);
    showMsg('success', t('update_success'), 6000);
  } catch (e) {
    showMsg('error', `${e}`, 12000);
  } finally {
    isRunning.value = false;
  }
}
</script>
