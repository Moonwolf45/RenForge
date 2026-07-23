<template>
  <div class="toast-stack">
    <transition-group name="toast">
      <div v-for="tt in toasts" :key="tt.id" class="toast" :class="'toast-' + tt.type">
        <span class="toast-ic"><Icon :name="tt.type === 'error' ? 'info' : tt.type === 'warn' ? 'alert' : 'check'" :size="16" :stroke-width="2.4" /></span>
        <span class="toast-text">{{ tt.text }}</span>
        <button v-if="tt.sticky && isExporting" class="btn btn-secondary toast-cancel" @click="cancelExport">{{ t('cancel_btn') }}</button>
        <button class="toast-close" @click="removeToast(tt.id)" :title="t('close')"><Icon name="x" :size="14" /></button>
      </div>
    </transition-group>
  </div>
</template>

<script setup>
import { toasts, removeToast, isExporting } from '../store.js';
import { cancelExport } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';
</script>

<style scoped>
.toast-stack {
  position: fixed; top: 64px; right: 16px; z-index: 2000;
  display: flex; flex-direction: column; gap: 10px;
  pointer-events: none; max-width: min(440px, 92vw);
}
.toast {
  pointer-events: auto;
  display: flex; align-items: center; gap: 10px;
  padding: 11px 12px 11px 14px;
  border-radius: var(--radius-lg, 10px);
  background: var(--bg-app);
  border: 1px solid var(--border-main);
  border-left: 4px solid var(--text-muted);
  box-shadow: 0 10px 28px -10px rgba(0,0,0,.45);
  font-size: 13px; color: var(--text-main);
}
.toast-success { border-left-color: var(--success-text); }
.toast-error { border-left-color: var(--error-text); }
.toast-warn { border-left-color: #eab308; }
.toast-ic { display: inline-flex; flex-shrink: 0; }
.toast-success .toast-ic { color: var(--success-text); }
.toast-error .toast-ic { color: var(--error-text); }
.toast-warn .toast-ic { color: #eab308; }
.toast-text { flex: 1; min-width: 0; white-space: pre-wrap; word-break: break-word; line-height: 1.4; }
.toast-cancel { padding: 3px 10px; font-size: 12px; flex-shrink: 0; }
.toast-close { background: none; border: none; color: var(--text-muted); cursor: pointer; padding: 3px; display: inline-flex; border-radius: 5px; flex-shrink: 0; transition: 0.15s; }
.toast-close:hover { color: var(--text-main); background: var(--bg-base); }

.toast-enter-active, .toast-leave-active { transition: transform 0.22s ease, opacity 0.22s ease; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateX(24px); }
@media (prefers-reduced-motion: reduce) {
  .toast-enter-active, .toast-leave-active { transition: none; }
}
</style>
