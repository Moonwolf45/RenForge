<template>
  <div v-if="translationPairs.length > 0" class="pairs-widget">
    <span class="pairs-label">{{ t('pairs_label') }}</span>
    <div class="pairs-list">
      <div
        v-for="p in translationPairs"
        :key="p.pair || 'legacy'"
        class="pair-chip"
        :class="{ active: p.is_active }"
        :title="pairTitle(p)"
        @click="switchPair(p)"
      >
        <span class="pair-name">
          <template v-if="p.is_legacy">legacy</template>
          <template v-else>{{ p.source || '?' }} <span class="pair-arrow">→</span> {{ p.target || '?' }}</template>
        </span>
        <span class="pair-prog">{{ p.translated }}/{{ p.total }}</span>
        <span v-if="p.is_built && p.is_dirty" class="pair-dirty" :title="t('pair_dirty_hint')">●</span>
        <span v-if="p.is_built" class="pair-export-wrap">
          <button
            class="pair-export"
            :title="t('export_translation')"
            @click.stop="toggleExportMenu(p)"
          >{{ t('export_btn') }}</button>
          <div v-if="exportMenuFor === (p.pair || 'legacy')" class="export-menu" @click.stop>
            <button class="export-opt" @click="doExport(p, 'full')">
              <span class="export-opt-title">{{ t('export_full') }}</span>
              <span class="export-opt-hint">{{ t('export_full_hint') }}</span>
            </button>
            <button class="export-opt" @click="doExport(p, 'mod')">
              <span class="export-opt-title">{{ t('export_mod') }}</span>
              <span class="export-opt-hint">{{ t('export_mod_hint') }}</span>
            </button>
            <button class="export-opt export-opt-danger" @click="doRemove(p)">
              <span class="export-opt-title">{{ t('remove_mod') }}</span>
              <span class="export-opt-hint">{{ t('remove_mod_hint') }}</span>
            </button>
          </div>
        </span>
        <button
          v-if="!p.is_legacy"
          class="pair-del"
          :title="t('pairs_delete')"
          @click.stop="deletePair(p)"
        >×</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { translationPairs } from '../store.js';
import { switchPair, deletePair, exportTranslation, removeMod } from '../actions.js';
import { t } from '../locales.js';

const exportMenuFor = ref(null);

function toggleExportMenu(p) {
  const key = p.pair || 'legacy';
  exportMenuFor.value = exportMenuFor.value === key ? null : key;
}

function doExport(p, mode) {
  exportMenuFor.value = null;
  exportTranslation(p, mode);
}

function doRemove(p) {
  exportMenuFor.value = null;
  removeMod(p);
}

function closeMenu() { exportMenuFor.value = null; }
onMounted(() => document.addEventListener('click', closeMenu));
onUnmounted(() => document.removeEventListener('click', closeMenu));

function pairTitle(p) {
  if (p.is_legacy) return t('pairs_legacy_hint');
  const pct = p.total > 0 ? Math.round((p.translated / p.total) * 100) : 0;
  return `${p.source} → ${p.target} — ${p.translated}/${p.total} (${pct}%)`;
}
</script>

<style scoped>
.pairs-widget {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.pairs-label {
  font-size: 12px;
  color: var(--text-secondary, #888);
  white-space: nowrap;
}
.pairs-list {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}
.pair-chip {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  border-radius: 14px;
  background: var(--bg-panel);
  border: 1px solid var(--border-input);
  cursor: pointer;
  font-size: 12px;
  transition: border-color .15s, background .15s;
  max-width: 260px;
}
.pair-chip:hover { border-color: var(--accent, #4ea1d3); }
.pair-chip.active {
  border-color: var(--accent, #4ea1d3);
  background: color-mix(in srgb, var(--accent, #4ea1d3) 18%, transparent);
}
.pair-name {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pair-arrow { color: var(--accent, #4ea1d3); }
.pair-prog {
  color: var(--text-secondary, #888);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.pair-del {
  border: none;
  background: transparent;
  color: var(--text-secondary, #888);
  cursor: pointer;
  font-size: 15px;
  line-height: 1;
  padding: 0 2px;
  border-radius: 4px;
}
.pair-del:hover { color: #e05a5a; background: rgba(224,90,90,.12); }

.pair-export-wrap { position: relative; display: inline-flex; }
.pair-dirty { color: #eab308; font-size: 11px; line-height: 1; cursor: default; }
.pair-export {
  border: 1px solid var(--border-input);
  background: transparent;
  color: var(--text-secondary, #aaa);
  cursor: pointer;
  font-size: 11px;
  line-height: 1;
  padding: 3px 7px;
  border-radius: 8px;
  white-space: nowrap;
}
.pair-export:hover {
  color: var(--accent, #4ea1d3);
  border-color: var(--accent, #4ea1d3);
}
.export-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 50;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px;
  width: 260px;
  background: var(--bg-panel);
  border: 1px solid var(--border-main);
  border-radius: 10px;
  box-shadow: 0 8px 24px rgba(0,0,0,.35);
  cursor: default;
}
.export-opt {
  display: flex;
  flex-direction: column;
  gap: 2px;
  text-align: left;
  padding: 8px 10px;
  border: none;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
}
.export-opt:hover { background: color-mix(in srgb, var(--accent, #4ea1d3) 16%, transparent); }
.export-opt-title { font-size: 12px; font-weight: 600; color: var(--text-main); }
.export-opt-hint { font-size: 11px; color: var(--text-secondary); line-height: 1.35; white-space: normal; }
.export-opt-danger { border-top: 1px solid var(--border-main); margin-top: 2px; }
.export-opt-danger:hover { background: rgba(224,90,90,.14); }
.export-opt-danger .export-opt-title { color: #e05a5a; }
</style>
