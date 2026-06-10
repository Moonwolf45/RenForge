<template>
  <div class="empty-state-box">
    <span class="empty-state-ic"><Icon :name="icon" :size="46" :stroke-width="1.4" /></span>
    <div class="empty-state-title">{{ title }}</div>
    <div v-if="hint" class="empty-state-hint">{{ hint }}</div>
    <slot />
  </div>
</template>

<script setup>
import Icon from './Icon.vue';

defineProps({
  icon: { type: String, default: 'info' },
  title: { type: String, default: '' },
  hint: { type: String, default: '' },
});
</script>

<style scoped>
.empty-state-box {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  text-align: center; gap: 10px; padding: 48px 24px; color: var(--text-muted);
}
.empty-state-ic { color: var(--text-muted); opacity: 0.6; margin-bottom: 2px; }
.empty-state-title { font-size: 16px; font-weight: 600; color: var(--text-secondary); }
.empty-state-hint { font-size: 13px; color: var(--text-muted); max-width: 440px; line-height: 1.5; }
</style>
