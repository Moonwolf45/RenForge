<template>
  <div v-if="translationPairs.length > 0" class="pairs-widget">
    <span class="pairs-label">{{ t('pairs_label') }}</span>
    <div class="pairs-list">
      <div
        v-for="p in translationPairs"
        :key="p.pair || 'legacy'"
        class="pair-card"
        :class="{ active: p.is_active }"
        :title="pairTitle(p)"
        @click="switchPair(p)"
      >
        <div class="pc-head">
          <span class="pc-name">
            <template v-if="p.is_legacy">legacy</template>
            <template v-else>{{ p.source || '?' }} <span class="pc-arrow">→</span> {{ p.target || '?' }}</template>
          </span>
          <span class="pc-status" :class="'st-' + statusKey(p)">{{ statusLabel(p) }}</span>
        </div>

        <div class="pc-bar"><div class="pc-bar-fill" :style="{ width: pct(p) + '%' }"></div></div>

        <div class="pc-foot">
          <span class="pc-prog">{{ p.translated }} / {{ p.total }} <span class="pc-pct">{{ pct(p) }}%</span></span>
          <span class="pc-actions">
            <span class="pc-export-wrap">
              <button class="pc-act" :title="t('export_translation')" @click.stop="toggleExportMenu(p)">
                <Icon name="download" :size="15" />
              </button>
              <div v-if="exportMenuFor === (p.pair || 'legacy')" class="export-menu" @click.stop>
                <button class="export-opt" @click="doExportStrings(p)">
                  <span class="export-opt-title">{{ t('export_strings') }}</span>
                  <span class="export-opt-hint">{{ t('export_strings_hint') }}</span>
                </button>
                <template v-if="p.is_built">
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
                </template>
              </div>
            </span>
            <button
              v-if="!p.is_legacy"
              class="pc-act pc-del"
              :title="t('pairs_delete')"
              @click.stop="deletePair(p)"
            >
              <Icon name="trash" :size="15" />
            </button>
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { translationPairs } from '../store.js';
import { switchPair, deletePair, exportTranslation, removeMod, exportAllStrings } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const exportMenuFor = ref(null);

function toggleExportMenu(p) {
  const key = p.pair || 'legacy';
  exportMenuFor.value = exportMenuFor.value === key ? null : key;
}

function doExport(p, mode) {
  exportMenuFor.value = null;
  exportTranslation(p, mode);
}

async function doExportStrings(p) {
  exportMenuFor.value = null;
  // Экспорт читает БД активной пары — переключаемся на выбранную, если нужно.
  if (!p.is_active) await switchPair(p);
  await exportAllStrings();
}

function doRemove(p) {
  exportMenuFor.value = null;
  removeMod(p);
}

function closeMenu() { exportMenuFor.value = null; }
onMounted(() => document.addEventListener('click', closeMenu));
onUnmounted(() => document.removeEventListener('click', closeMenu));

function pct(p) {
  return p.total > 0 ? Math.round((p.translated / p.total) * 100) : 0;
}

// Статус пары: черновик (не собрано) / собрано / изменено (собрано, но БД менялась).
function statusKey(p) {
  if (p.is_built && p.is_dirty) return 'dirty';
  if (p.is_built) return 'built';
  return 'draft';
}
function statusLabel(p) {
  return t('pair_status_' + statusKey(p));
}

function pairTitle(p) {
  if (p.is_legacy) return t('pairs_legacy_hint');
  return `${p.source} → ${p.target} — ${p.translated}/${p.total} (${pct(p)}%)`;
}
</script>

<style scoped>
.pairs-widget {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.pairs-label {
  font-size: 12px;
  color: var(--text-secondary, #888);
  white-space: nowrap;
  margin-top: 8px;
}
.pairs-list {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  min-width: 0;
}

.pair-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 220px;
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--bg-panel);
  border: 1px solid var(--border-input);
  cursor: pointer;
  transition: border-color .15s, background .15s, box-shadow .15s;
}
.pair-card:hover { border-color: var(--accent, #4ea1d3); }
.pair-card.active {
  border-color: var(--accent, #4ea1d3);
  background: color-mix(in srgb, var(--accent, #4ea1d3) 12%, transparent);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent, #4ea1d3) 40%, transparent) inset;
}

.pc-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.pc-name {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-weight: 600;
  font-size: 13px;
  color: var(--text-main);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pc-arrow { color: var(--accent, #4ea1d3); }

.pc-status {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: .4px;
  padding: 2px 7px;
  border-radius: 999px;
  white-space: nowrap;
  flex-shrink: 0;
}
.st-draft { color: var(--text-secondary, #888); background: color-mix(in srgb, var(--text-secondary, #888) 16%, transparent); }
.st-built { color: #3fae6a; background: rgba(63, 174, 106, .15); }
.st-dirty { color: #eab308; background: rgba(234, 179, 8, .16); }

.pc-bar {
  height: 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text-secondary, #888) 22%, transparent);
  overflow: hidden;
}
.pc-bar-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent, #4ea1d3);
  transition: width .3s ease;
}

.pc-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.pc-prog {
  font-size: 12px;
  color: var(--text-secondary, #888);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.pc-pct { color: var(--text-main); font-weight: 600; margin-left: 2px; }

.pc-actions {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  opacity: .55;
  transition: opacity .15s;
}
.pair-card:hover .pc-actions { opacity: 1; }

.pc-export-wrap { position: relative; display: inline-flex; }
.pc-act {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: 1px solid var(--border-input);
  background: transparent;
  color: var(--text-secondary, #aaa);
  cursor: pointer;
  border-radius: 7px;
  transition: color .15s, border-color .15s, background .15s;
}
.pc-act:hover { color: var(--accent, #4ea1d3); border-color: var(--accent, #4ea1d3); }
.pc-del:hover { color: #e05a5a; border-color: #e05a5a; background: rgba(224, 90, 90, .12); }

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
