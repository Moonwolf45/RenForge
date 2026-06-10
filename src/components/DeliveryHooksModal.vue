<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content hooks-modal">
      <div class="modal-header">
        <h2><Icon name="code" :size="16" /> {{ t('hooks_title') }}</h2>
        <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
      </div>

      <div class="hooks-body">
        <p class="hooks-intro"><Icon name="info" :size="15" /><span>{{ t('hooks_intro') }}</span></p>

        <div class="hooks-api">
          <button class="hooks-api-head" @click="apiOpen = !apiOpen">
            <Icon :name="apiOpen ? 'eye' : 'eye-off'" :size="14" /> {{ t('hooks_api_title') }}
          </button>
          <div v-if="apiOpen" class="hooks-api-body">
            <code>renforge_tr(s)</code> — {{ t('hooks_api_tr') }}<br>
            <code>renforge_wrap("renpy.foo.bar", arg=0)</code> — {{ t('hooks_api_wrap') }}<br>
            <code>renforge_wrap_ret("...")</code> — {{ t('hooks_api_wrapret') }}<br>
            <code>renforge_filter(func)</code> — {{ t('hooks_api_filter') }}<br>
            <code>renforge_add(orig, tran)</code> — {{ t('hooks_api_add') }}
            <p class="hooks-api-phase">{{ t('hooks_api_phasenote') }}</p>
          </div>
        </div>

        <div v-if="!hooks.length" class="hooks-empty">{{ t('hooks_empty') }}</div>

        <div v-for="(h, i) in hooks" :key="i" class="hook-card" :class="{ disabled: !h.enabled }">
          <div class="hook-row">
            <label class="hook-en" :title="t('hooks_enabled')"><input type="checkbox" v-model="h.enabled" /></label>
            <input class="hook-name" v-model="h.name" :placeholder="t('hooks_name_ph')" />
            <select class="hook-phase" v-model="h.phase" :title="t('hooks_phase')">
              <option value="init">init</option>
              <option value="early">early</option>
            </select>
            <select class="hook-phase" v-model="h.scope" :title="t('hooks_scope')">
              <option value="global">{{ t('hooks_scope_global') }}</option>
              <option value="project">{{ t('hooks_scope_project') }}</option>
            </select>
            <button class="icon-text-btn" @click="move(i, -1)" :disabled="i === 0" :title="t('move_up')"><Icon name="arrow-down" :size="14" style="transform: rotate(180deg);" /></button>
            <button class="icon-text-btn" @click="move(i, 1)" :disabled="i === hooks.length - 1" :title="t('move_down')"><Icon name="arrow-down" :size="14" /></button>
            <button class="icon-text-btn icon-danger" @click="remove(i)" :title="t('manual_delete')"><Icon name="trash" :size="14" /></button>
          </div>
          <textarea class="hook-code" v-model="h.code" rows="6" spellcheck="false" :placeholder="t('hooks_code_ph')"></textarea>
          <div class="hook-foot">
            <select class="hook-tpl" @change="insertTemplate(i, $event.target.value); $event.target.value=''">
              <option value="">{{ t('hooks_insert_tpl') }}</option>
              <option value="wrap">{{ t('hooks_tpl_wrap') }}</option>
              <option value="filter">{{ t('hooks_tpl_filter') }}</option>
              <option value="patch">{{ t('hooks_tpl_patch') }}</option>
            </select>
          </div>
        </div>

        <button class="btn btn-secondary hooks-add" @click="addHook"><Icon name="plus" :size="14" /> {{ t('hooks_add') }}</button>
      </div>

      <div class="hooks-footer">
        <span v-if="status" class="hooks-status" :class="status.type">
          <Icon :name="status.type === 'error' ? 'info' : 'check'" :size="14" /> {{ status.text }}
        </span>
        <span class="hooks-foot-spacer"></span>
        <button class="btn btn-secondary" @click="check" :disabled="busy">{{ t('hooks_check') }}</button>
        <button class="btn btn-secondary" @click="close">{{ t('close') }}</button>
        <button class="btn btn-primary" @click="save" :disabled="busy">{{ t('save') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { showDeliveryHooksModal, projectPath, showMsg } from '../store.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const hooks = ref([]);
const status = ref(null);
const busy = ref(false);
const apiOpen = ref(false);

const TEMPLATES = {
  wrap: '# Игра рисует текст через свою функцию — направим её через перевод.\nrenforge_wrap("renpy.foo.show_caption")',
  filter: '# Свой фильтр для диалогов/меню.\ndef my_filter(s):\n    return renforge_tr(s)\nrenforge_filter(my_filter)',
  patch: '# Произвольный монкипатч с доступом к словарям.\n_orig = renpy.foo.bar\ndef _patched(*a, **k):\n    if a and isinstance(a[0], _rf_strtypes):\n        a = (renforge_tr(a[0]),) + a[1:]\n    return _orig(*a, **k)\nrenpy.foo.bar = _patched',
};

onMounted(async () => {
  try {
    const list = await invoke('get_delivery_hooks', { projectPath: projectPath.value });
    hooks.value = Array.isArray(list) ? list : [];
  } catch (e) { hooks.value = []; }
});

function addHook() {
  hooks.value.push({ name: t('hooks_new_name'), phase: 'init', enabled: true, code: '', scope: 'global' });
}
function remove(i) { hooks.value.splice(i, 1); }
function move(i, dir) {
  const j = i + dir;
  if (j < 0 || j >= hooks.value.length) return;
  const arr = hooks.value;
  [arr[i], arr[j]] = [arr[j], arr[i]];
}
function insertTemplate(i, key) {
  if (!key || !TEMPLATES[key]) return;
  const cur = hooks.value[i].code || '';
  hooks.value[i].code = cur ? cur.replace(/\s*$/, '') + '\n\n' + TEMPLATES[key] : TEMPLATES[key];
}

async function validateAll() {
  for (const h of hooks.value) {
    if (!h.enabled || !h.code.trim()) continue;
    try {
      await invoke('validate_delivery_hook', { projectPath: projectPath.value, code: h.code });
    } catch (e) {
      return { ok: false, name: h.name, msg: (e && e.toString) ? e.toString() : String(e) };
    }
  }
  return { ok: true };
}

async function check() {
  busy.value = true; status.value = null;
  const v = await validateAll();
  busy.value = false;
  status.value = v.ok
    ? { type: 'success', text: t('hooks_check_ok') }
    : { type: 'error', text: `${v.name || ''} — ${v.msg}` };
}

async function save() {
  busy.value = true; status.value = null;
  const v = await validateAll();
  if (!v.ok) {
    busy.value = false;
    status.value = { type: 'error', text: `${v.name || ''} — ${v.msg}` };
    return;
  }
  const payload = hooks.value.map(h => ({
    name: h.name || '', phase: h.phase === 'early' ? 'early' : 'init',
    enabled: !!h.enabled, code: h.code || '',
    scope: h.scope === 'project' ? 'project' : 'global',
  }));
  try {
    await invoke('save_delivery_hooks', { projectPath: projectPath.value, hooks: payload });
    showMsg('success', t('hooks_saved'));
    close();
  } catch (e) {
    status.value = { type: 'error', text: (e && e.toString) ? e.toString() : String(e) };
  } finally {
    busy.value = false;
  }
}

function close() { showDeliveryHooksModal.value = false; }
</script>

<style scoped>
.hooks-modal { width: 720px; max-width: 95vw; display: flex; flex-direction: column; max-height: 88vh; }
.hooks-body { padding: 18px 20px; display: flex; flex-direction: column; gap: 14px; overflow-y: auto; }

.hooks-intro {
  margin: 0; display: flex; gap: 9px; align-items: flex-start; font-size: 13px; line-height: 1.5;
  color: var(--text-secondary);
  background: color-mix(in srgb, var(--status-review) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--status-review) 30%, transparent);
  border-radius: var(--radius-md, 8px); padding: 11px 13px;
}
.hooks-intro :deep(.rf-icon) { color: var(--status-review); flex-shrink: 0; margin-top: 1px; }

.hooks-api { border: 1px solid var(--border-main); border-radius: var(--radius-md, 8px); overflow: hidden; }
.hooks-api-head { width: 100%; display: flex; align-items: center; gap: 8px; background: var(--bg-panel); border: none; padding: 9px 12px; cursor: pointer; color: var(--text-main); font-size: 13px; font-weight: 600; }
.hooks-api-body { padding: 11px 13px; border-top: 1px solid var(--border-main); font-size: 12.5px; line-height: 1.9; color: var(--text-secondary); }
.hooks-api-body code { background: var(--code-bg); color: var(--syn-fn, var(--accent)); padding: 1px 6px; border-radius: 4px; font-size: 12px; }
.hooks-api-phase { margin: 9px 0 0; padding-top: 8px; border-top: 1px solid var(--border-main); font-size: 12px; color: var(--text-muted); }

.hooks-empty { color: var(--text-muted); font-size: 13px; text-align: center; padding: 8px; }

.hook-card { border: 1px solid var(--border-main); border-radius: var(--radius-md, 8px); padding: 12px; display: flex; flex-direction: column; gap: 9px; background: var(--bg-panel); }
.hook-card.disabled { opacity: 0.55; }
.hook-row { display: flex; align-items: center; gap: 8px; }
.hook-en { display: inline-flex; align-items: center; }
.hook-en input { margin: 0; }
.hook-name { flex: 1; min-width: 0; background: var(--bg-input); color: var(--text-main); border: 1px solid var(--border-input); border-radius: 6px; padding: 6px 9px; font-size: 13px; font-weight: 600; }
.hook-name:focus { outline: none; border-color: var(--accent); }
.hook-phase { background: var(--bg-input); color: var(--text-main); border: 1px solid var(--border-input); border-radius: 6px; padding: 6px 8px; font-size: 12px; }
.hook-code {
  width: 100%; box-sizing: border-box; resize: vertical;
  font-family: ui-monospace, 'Cascadia Code', Consolas, monospace; font-size: 12.5px; line-height: 1.5;
  background: var(--code-bg); color: var(--code-text);
  border: 1px solid var(--border-input); border-radius: var(--radius-sm, 6px); padding: 9px 11px; tab-size: 4;
}
.hook-code:focus { outline: none; border-color: var(--accent); }
.hook-foot { display: flex; }
.hook-tpl { background: var(--bg-input); color: var(--text-muted); border: 1px solid var(--border-input); border-radius: 6px; padding: 4px 8px; font-size: 12px; }

.hooks-add { align-self: flex-start; display: inline-flex; align-items: center; gap: 6px; }

.hooks-footer { display: flex; align-items: center; gap: 10px; padding: 13px 20px; border-top: 1px solid var(--border-main); background: var(--bg-panel); border-bottom-left-radius: inherit; border-bottom-right-radius: inherit; }
.hooks-foot-spacer { flex: 1; }
.hooks-status { display: inline-flex; align-items: center; gap: 6px; font-size: 12.5px; max-width: 60%; }
.hooks-status.success { color: var(--success-text); }
.hooks-status.error { color: var(--error-text); }
</style>
