<template>
  <div class="accent-picker">
    <div class="accent-presets">
      <button
        v-for="(c, key) in ACCENTS"
        :key="key"
        class="accent-swatch"
        :class="{ active: uiAccent === key }"
        :style="{ background: c.c }"
        @click="pick(key)"
        :title="key"
      ></button>
    </div>
    <div class="accent-custom-row">
      <div class="accent-edit" :title="t('accent_custom')" @click="openPicker">
        <span class="accent-swatch accent-edit-dot" :class="{ active: isCustom }" :style="{ background: dotColor }"></span>
        <span class="accent-edit-ic"><Icon name="edit" :size="14" /></span>
        <input ref="colorInput" type="color" class="accent-color-native" :value="dotColor" @input="pickHex($event.target.value)" />
      </div>
      <input
        type="text"
        class="accent-hex"
        :value="currentColor"
        @change="pickHex($event.target.value)"
        @keyup.enter="pickHex($event.target.value)"
        maxlength="7"
        spellcheck="false"
        placeholder="#5b82c9"
      />
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import { uiAccent, ACCENTS, resolveAccent } from '../store.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const currentColor = computed(() => resolveAccent(uiAccent.value).c);
const isCustom = computed(() => !ACCENTS[uiAccent.value]);

// Случайный «затравочный» цвет для кружка, пока пользователь не выбрал свой.
function randHex() { return '#' + Math.floor(Math.random() * 0xffffff).toString(16).padStart(6, '0'); }
const seedColor = ref(randHex());
const dotColor = computed(() => (isCustom.value ? currentColor.value : seedColor.value));

const colorInput = ref(null);
function openPicker() { if (colorInput.value) colorInput.value.click(); }

function persist() { localStorage.setItem('renforge_ui_accent', uiAccent.value); }
function pick(key) { uiAccent.value = key; persist(); }
function pickHex(v) {
  let s = (v || '').trim();
  if (s && !s.startsWith('#')) s = '#' + s;
  if (/^#[0-9a-fA-F]{6}$/.test(s)) { uiAccent.value = s.toLowerCase(); persist(); }
}
</script>

<style scoped>
.accent-picker {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
}
.accent-presets {
  display: flex;
  gap: 7px;
  flex-wrap: wrap;
  justify-content: flex-end;
  max-width: 200px;
}
.accent-custom-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.accent-edit {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.accent-edit-ic {
  display: inline-flex;
  color: var(--text-muted);
  transition: color 0.15s;
}
.accent-edit:hover .accent-edit-ic { color: var(--text-main); }
/* Нативный color input убран из видимой зоны (0×0): в этом WebView он рисует свой
   свотч-прямоугольник при любых opacity/appearance. Палитру открываем colorInput.click()
   по клику на кружок/карандаш; инпут стоит у левого нижнего угла, чтобы попап лёг рядом. */
.accent-color-native {
  position: absolute;
  left: 0; bottom: 0;
  width: 0; height: 0;
  opacity: 0; border: 0; padding: 0; margin: 0;
  overflow: hidden; pointer-events: none;
}
.accent-hex {
  width: 86px; padding: 6px 6px;
  font-size: 12px; font-family: ui-monospace, "Cascadia Code", monospace; text-align: center;
  background: var(--bg-input); border: 1px solid var(--border-input);
  border-radius: 6px; color: var(--text-main); outline: none; transition: 0.15s;
}
.accent-hex:focus { border-color: var(--accent); background: var(--bg-input-focus); }
</style>
