<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content asm-modal">
      <div class="modal-header">
        <h2><Icon :name="editMode ? 'edit' : 'plus'" :size="16" /> {{ editMode ? t('edit_string') : t('add_string') }}</h2>
        <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
      </div>

      <div class="asm-body">
        <p v-if="!editMode" class="asm-intro"><Icon name="info" :size="15" /><span>{{ t('add_string_hint') }}</span></p>

        <!-- Тип / канал -->
        <div class="asm-field">
          <span class="asm-label">{{ t('manual_string_type') }}</span>
          <div class="asm-seg-group">
            <button class="asm-seg" :class="{ active: type === 'dialogue' }" @click="type = 'dialogue'">
              <Icon name="translate" :size="16" /><span>{{ t('mtype_dialogue') }}</span>
            </button>
            <button class="asm-seg" :class="{ active: type === 'ui' }" @click="type = 'ui'">
              <Icon name="database" :size="16" /><span>{{ t('mtype_ui') }}</span>
            </button>
          </div>
          <span class="asm-help">{{ type === 'dialogue' ? t('mtype_dialogue_help') : t('mtype_ui_help') }}</span>
        </div>

        <!-- Куда добавить (только при добавлении и если открыт реальный файл) -->
        <div class="asm-field" v-if="!editMode && inFile">
          <span class="asm-label">{{ t('manual_target') }}</span>
          <div class="asm-dest">
            <div class="asm-seg-group asm-dest-seg">
              <button class="asm-seg" :class="{ active: toCurrent }" @click="toCurrent = true">
                <Icon name="file" :size="15" /><span class="asm-ellip">{{ currentName }}</span>
              </button>
              <button class="asm-seg" :class="{ active: !toCurrent }" @click="toCurrent = false">
                <Icon name="plus" :size="15" /><span>{{ t('manual_strings_file') }}</span>
              </button>
            </div>
            <div class="asm-pos" v-if="toCurrent">
              <span class="asm-pos-label">{{ t('manual_position') }}</span>
              <input type="number" min="0" v-model.number="position" class="asm-num" />
            </div>
          </div>
          <span class="asm-help">{{ toCurrent ? t('manual_position_hint') : t('manual_saves_to') }}</span>
        </div>

        <!-- Оригинал -->
        <div class="asm-field">
          <span class="asm-label">{{ t('manual_string_orig') }}</span>
          <textarea ref="origEl" v-model="original" class="asm-input" rows="2" :placeholder="t('manual_string_orig_ph')"></textarea>
        </div>

        <!-- Перевод -->
        <div class="asm-field">
          <span class="asm-label">{{ t('manual_string_tran') }}</span>
          <textarea v-model="translation" class="asm-input" rows="2" :placeholder="t('manual_string_tran_ph')" @keydown.ctrl.enter="submit"></textarea>
        </div>
      </div>

      <div class="asm-footer">
        <span v-if="!editMode && addedCount > 0" class="asm-added"><Icon name="check" :size="14" /> {{ t('manual_added') }} {{ addedCount }}</span>
        <span class="asm-foot-spacer"></span>
        <button class="btn btn-secondary" @click="close">{{ editMode ? t('close') : t('done') }}</button>
        <button class="btn btn-primary" :disabled="!original.trim()" @click="submit">{{ editMode ? t('save') : t('manual_add_btn') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, nextTick } from 'vue';
import { showAddStringModal, editorResizeTick, currentFilePath, MANUAL_FILE, getFileName, manualEditTarget } from '../store.js';
import { addManualString, updateManualString } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const editMode = computed(() => !!manualEditTarget.value);
const inFile = computed(() => currentFilePath.value && currentFilePath.value !== MANUAL_FILE);
const currentName = computed(() => getFileName(currentFilePath.value));

const type = ref('dialogue');
const original = ref('');
const translation = ref('');
const position = ref(0);
const toCurrent = ref(inFile.value); // по умолчанию — в текущий файл, если он открыт
const addedCount = ref(0);
const origEl = ref(null);

// Предзаполнение при редактировании.
if (manualEditTarget.value) {
  const b = manualEditTarget.value;
  type.value = (b.block_type === 'dialogue' || b.block_type === 'menu') ? 'dialogue' : 'ui';
  original.value = b.original || '';
  translation.value = b.translation || '';
}

async function submit() {
  if (editMode.value) {
    updateManualString(manualEditTarget.value, original.value, translation.value, type.value);
    editorResizeTick.value++;
    close();
    return;
  }
  const id = await addManualString(original.value, translation.value, type.value, toCurrent.value, position.value);
  if (!id) return;
  addedCount.value++;
  original.value = '';
  translation.value = '';
  editorResizeTick.value++;
  await nextTick();
  if (origEl.value) origEl.value.focus();
  const el = document.getElementById('block-' + id);
  if (el) el.scrollIntoView({ block: 'center', behavior: 'smooth' });
}

function close() {
  manualEditTarget.value = null;
  showAddStringModal.value = false;
}
</script>

<style scoped>
.asm-modal { width: 580px; max-width: 92vw; display: flex; flex-direction: column; max-height: 88vh; }

.asm-body { padding: 20px 22px; display: flex; flex-direction: column; gap: 18px; overflow-y: auto; }

.asm-intro {
  margin: 0; display: flex; gap: 9px; align-items: flex-start;
  font-size: 13px; line-height: 1.5; color: var(--text-secondary);
  background: color-mix(in srgb, var(--accent) 8%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
  border-radius: var(--radius-md, 8px); padding: 11px 13px;
}
.asm-intro :deep(.rf-icon) { color: var(--accent); flex-shrink: 0; margin-top: 1px; }

.asm-field { display: flex; flex-direction: column; gap: 8px; }
.asm-label {
  font-size: 11px; font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase;
  color: var(--text-muted);
}
.asm-help { font-size: 12px; line-height: 1.4; color: var(--text-muted); }

/* Сегментированные переключатели на всю ширину, равные доли */
.asm-seg-group {
  display: flex; gap: 4px; padding: 4px;
  background: var(--bg-base); border: 1px solid var(--border-input);
  border-radius: var(--radius-md, 8px);
}
.asm-seg {
  flex: 1 1 0; min-width: 0; display: inline-flex; align-items: center; justify-content: center; gap: 7px;
  padding: 8px 10px; font-size: 13px; font-weight: 600;
  background: transparent; color: var(--text-secondary);
  border: none; border-radius: var(--radius-sm, 6px); cursor: pointer; transition: 0.15s;
}
.asm-seg:hover { color: var(--text-main); background: color-mix(in srgb, var(--text-muted) 10%, transparent); }
.asm-seg.active { background: var(--accent); color: var(--accent-contrast, #fff); }
.asm-seg.active :deep(.rf-icon) { color: var(--accent-contrast, #fff); }
.asm-ellip { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.asm-dest { display: flex; align-items: center; gap: 12px; }
.asm-dest-seg { flex: 1 1 auto; min-width: 0; }
.asm-pos { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
.asm-pos-label { font-size: 12px; color: var(--text-secondary); }
.asm-num {
  width: 74px; box-sizing: border-box; background: var(--bg-input); color: var(--text-main);
  border: 1px solid var(--border-input); border-radius: var(--radius-sm, 6px);
  padding: 7px 9px; font-size: 14px; font-variant-numeric: tabular-nums;
}
.asm-num:focus { outline: none; border-color: var(--accent); }

.asm-input {
  width: 100%; box-sizing: border-box; resize: vertical; min-height: 44px;
  background: var(--bg-input); color: var(--text-main);
  border: 1px solid var(--border-input); border-radius: var(--radius-md, 8px);
  padding: 10px 12px; font-size: 14px; font-family: inherit; line-height: 1.5;
}
.asm-input:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent); }

.asm-footer {
  display: flex; align-items: center; gap: 10px;
  padding: 14px 22px; border-top: 1px solid var(--border-main); background: var(--bg-panel);
  border-bottom-left-radius: inherit; border-bottom-right-radius: inherit;
}
.asm-foot-spacer { flex: 1; }
.asm-added { display: inline-flex; align-items: center; gap: 6px; font-size: 12.5px; font-weight: 600; color: var(--success-text); }
</style>
