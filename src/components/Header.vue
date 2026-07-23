<template>
  <header class="header">
    <!-- LEFT SECTION -->
    <div class="header-section">
      <template v-if="['dashboard', 'gallery', 'audio'].includes(currentMode)">
        <div class="logo" style="margin-right: 15px; cursor: pointer; user-select: none;" @click="onLogoClick"><img :src="appLogo" class="logo-img" alt="" /><span class="logo-text"><span class="logo-ren">Ren</span><span class="logo-forge">Forge</span><sup class="version">1.2</sup></span></div>
        
        <div class="popover-wrapper">
          <button class="btn btn-secondary" style="display:inline-flex; align-items:center; justify-content:center;" @click="togglePopover('settings')" :class="{active: activePopover === 'settings'}" :title="t('settings')"><Icon name="gear" :size="18" /></button>
          <div v-if="activePopover === 'settings'" class="popover-menu">
            <div class="setting-row">
                <label>{{ t('ui_theme') }}</label>
                <select class="settings-select" v-model="uiTheme" @change="saveSettings">
                    <option value="dark">{{ t('theme_dark') }}</option>
                    <option value="black">{{ t('theme_black') }}</option>
                    <option value="light">{{ t('theme_light') }}</option>
                </select>
            </div>
            <div class="setting-row">
                <label>{{ t('ui_accent') }}</label>
                <AccentPicker />
            </div>
            <div class="setting-row">
                <label>{{ t('ui_lang') }}</label>
                <select class="settings-select" v-model="uiLang" @change="saveSettings">
                    <option value="ru">Русский</option>
                    <option value="en">English</option>
                    <option value="zh">中文</option>
                    <option value="ja">日本語</option>
                    <option value="es">Español</option>
                    <option value="pt">Português</option>
                </select>
            </div>
            <div class="setting-row">
                <label>{{ t('translate_to') }}</label>
                <div style="display:flex; flex-direction:column; gap:8px; align-items:flex-end;">
                  <select class="settings-select" v-model="targetLangSelect" @change="onTargetLangSelect">
                      <option value="">{{ t('lang_not_set') }}</option>
                      <option value="russian">Русский (russian)</option>
                      <option value="english">English (english)</option>
                      <option value="spanish">Español (spanish)</option>
                      <option value="french">Français (french)</option>
                      <option value="german">Deutsch (german)</option>
                      <option value="custom">{{ t('custom_lang') }}</option>
                  </select>
                  <input v-if="targetLangSelect === 'custom'" type="text" class="settings-select" v-model="targetLang" @change="saveSettings" :placeholder="t('custom_lang_placeholder')" style="width: 100%; box-sizing: border-box;" />
                </div>
            </div>
            <div class="setting-row" v-if="projectPath">
                <label>{{ t('source_lang') }}</label>
                <select class="settings-select" v-model="sourceLang" @change="saveSettings">
                    <option value="">{{ t('lang_not_set') }}</option>
                    <option value="original">original</option>
                    <option v-for="lang in availableLanguages.filter(l => l !== 'original')" :key="lang" :value="lang">{{ lang }}</option>
                </select>
            </div>
            <div class="setting-row">
                <label>{{ t('target_script') }}</label>
                <select class="settings-select" v-model="targetScript" @change="saveSettings">
                    <option value="auto">{{ t('script_auto') }}</option>
                    <option v-for="code in SCRIPT_CODES" :key="code" :value="code">{{ t('script_' + code) }}</option>
                </select>
            </div>
            <div class="setting-row" style="border-bottom:none;">
              <button class="dropdown-item-btn" style="width:100%; display:inline-flex; align-items:center; gap:8px;" @click="showAboutModal = true; activePopover = null"><Icon name="info" :size="15" /> {{ t('about_title') }}</button>
            </div>
          </div>
        </div>
        
        <div class="popover-wrapper">
          <button class="btn btn-secondary" style="display:inline-flex; align-items:center; justify-content:center;" @click="togglePopover('help')" :class="{active: activePopover === 'help'}" :title="t('help_title')"><Icon name="help" :size="18" /></button>
          <div v-if="activePopover === 'help'" class="popover-menu" style="width: 450px;">
            <template v-if="currentMode === 'dashboard'">
              <ol style="margin: 0; padding-left: 15px; line-height: 1.6; font-size: 13px;">
                <li style="margin-bottom: 10px;" v-html="t('help_step1')"></li>
                <li style="margin-bottom: 10px;" v-html="t('help_step2')"></li>
                <li style="margin-bottom: 10px;" v-html="t('help_step3')"></li>
                <li style="margin-bottom: 10px;" v-html="t('help_step4')"></li>
                <li style="margin-bottom: 10px;" v-html="t('help_step5')"></li>
                <li style="margin-bottom: 10px;" v-html="t('help_step6')"></li>
                <li v-html="t('help_step7')"></li>
              </ol>
              <p class="help-note" style="margin: 12px 0 0; padding-top: 10px; border-top: 1px solid var(--border, rgba(128,128,128,0.25)); font-size: 12px; line-height: 1.5; opacity: 0.85;" v-html="t('help_note_langselect')"></p>
            </template>
            <template v-else-if="currentMode === 'gallery'">
              <p style="font-size: 13px; line-height: 1.5; margin: 0;">{{ t('help_img_desc') }}</p>
            </template>
            <template v-else-if="currentMode === 'audio'">
              <p style="font-size: 13px; line-height: 1.5; margin: 0;">{{ t('help_audio_desc') }}</p>
            </template>
          </div>
        </div>
      </template>

      <template v-else-if="currentMode === 'editor'">
        <button class="btn btn-secondary" @click="closeEditor">{{ t('back') }}</button>
        <button v-if="currentFilePath !== MANUAL_FILE" class="header-filename header-filename-btn" :title="t('source_view_hint')" @click="showSourceModal = true">
          <Icon name="file" :size="14" />
          {{ getFileName(currentFilePath) }}
        </button>
        <span v-else class="header-filename"><Icon name="file" :size="14" /> {{ t('manual_strings_file') }}</span>
      </template>
    </div>

    <!-- CENTER SECTION -->
    <div class="header-section center">
      <template v-if="['dashboard', 'gallery', 'audio'].includes(currentMode)">
        <div class="segmented-control">
          <button :class="['seg-btn', { active: currentMode === 'dashboard' }]" @click="currentMode = 'dashboard'">{{ t('text') }}</button>
          <button :class="['seg-btn', { active: currentMode === 'gallery' }]" @click="currentMode = 'gallery'">{{ t('images') }}</button>
          <button :class="['seg-btn', { active: currentMode === 'audio' }]" @click="currentMode = 'audio'">{{ t('audio') }}</button>
        </div>
      </template>

      <template v-else-if="currentMode === 'editor'">
        <div class="btn-group" v-if="parsedBlocks.length > 0 && !isEditorLoading">
          <button class="group-btn" @click="isAiModalOpen = true">{{ t('ai_assistant') }}</button>
          
          <div class="popover-wrapper" style="display: inline-flex;">
            <button class="group-btn" @click="togglePopover('export')" :class="{active: activePopover === 'export'}">{{ t('export_btn') }} ▾</button>
            <div v-if="activePopover === 'export'" class="popover-menu popover-menu-sm">
              <button class="dropdown-item-btn" @click="exportCSV">Export .CSV</button>
              <button class="dropdown-item-btn" @click="exportJSON">Export .JSON</button>
              <button class="dropdown-item-btn" @click="exportPO">Export .PO</button>
            </div>
          </div>

          <div class="popover-wrapper" style="display: inline-flex;">
            <button class="group-btn" @click="togglePopover('import')" :class="{active: activePopover === 'import'}">{{ t('import_btn') }} ▾</button>
            <div v-if="activePopover === 'import'" class="popover-menu popover-menu-sm">
              <button class="dropdown-item-btn" @click="importCSV">Import .CSV</button>
              <button class="dropdown-item-btn" @click="importJSON">Import .JSON</button>
              <button class="dropdown-item-btn" @click="importPO">Import .PO</button>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- RIGHT SECTION -->
    <div class="header-section right">
      <template v-if="['dashboard', 'gallery', 'audio'].includes(currentMode)">
        <button class="btn btn-primary" @click="openProjectFolder">{{ t('select_folder') }}</button>
      </template>
      
      <template v-else-if="currentMode === 'editor'">
        <div class="popover-wrapper" v-if="!isEditorLoading" style="display:inline-flex;">
          <button class="btn btn-secondary" :class="['qa-' + qaState, { active: activePopover === 'qa' }]" style="display:inline-flex; align-items:center; justify-content:center;" @click="togglePopover('qa')" :title="t('qa_menu')"><Icon :name="qaIcon" :size="18" /></button>
          <div v-if="activePopover === 'qa'" class="popover-menu popover-menu-sm popover-right">
            <button v-if="hasErrors" class="dropdown-item-btn" @click="jumpToNextError(); activePopover = null">{{ t('next_error') }}</button>
            <button v-if="hasReview" class="dropdown-item-btn" @click="jumpToNextReview(); activePopover = null">{{ t('next_review') }} ({{ reviewCount }})</button>
            <button v-if="hasWarnDiag" class="dropdown-item-btn" @click="jumpToNextWarning(); activePopover = null">{{ t('next_warning') }}</button>
            <button v-if="hasFixables" class="dropdown-item-btn" @click="fixFileAll(); activePopover = null">{{ t('diag_fix_file') }}</button>
            <div v-if="!hasErrors && !hasReview && !hasWarnDiag && !hasFixables" class="dropdown-empty">{{ t('qa_clean') }}</div>
          </div>
        </div>
        
        <button v-if="!isEditorLoading" class="btn btn-secondary header-add-string" style="display:inline-flex; align-items:center; justify-content:center;" @click="showAddStringModal = true" :title="t('add_string')"><Icon name="plus" :size="18" /></button>

        <div class="popover-wrapper" v-if="!isEditorLoading">
          <button class="btn btn-secondary" style="display:inline-flex; align-items:center; justify-content:center;" @click="togglePopover('settings')" :class="{active: activePopover === 'settings'}" :title="t('settings')"><Icon name="gear" :size="18" /></button>
          <div v-if="activePopover === 'settings'" class="popover-menu popover-right">
            <div class="setting-row">
                <label>{{ t('ui_theme') }}</label>
                <select class="settings-select" v-model="uiTheme" @change="saveSettings">
                    <option value="dark">{{ t('theme_dark') }}</option>
                    <option value="black">{{ t('theme_black') }}</option>
                    <option value="light">{{ t('theme_light') }}</option>
                </select>
            </div>
            <div class="setting-row">
                <label>{{ t('ui_accent') }}</label>
                <AccentPicker />
            </div>
            <div class="setting-row">
                <label>{{ t('ui_lang') }}</label>
                <select class="settings-select" v-model="uiLang" @change="saveSettings">
                    <option value="ru">Русский</option>
                    <option value="en">English</option>
                    <option value="zh">中文</option>
                    <option value="ja">日本語</option>
                    <option value="es">Español</option>
                    <option value="pt">Português</option>
                </select>
            </div>
            <div class="setting-row" style="border-bottom:none;">
              <button class="dropdown-item-btn" style="width:100%; display:inline-flex; align-items:center; gap:8px;" @click="showAboutModal = true; activePopover = null"><Icon name="info" :size="15" /> {{ t('about_title') }}</button>
            </div>
          </div>
        </div>
        
        <div class="popover-wrapper" v-if="!isEditorLoading">
          <button class="btn btn-secondary" style="display:inline-flex; align-items:center; justify-content:center;" @click="togglePopover('help')" :class="{active: activePopover === 'help'}" :title="t('help_title')"><Icon name="help" :size="18" /></button>
          <div v-if="activePopover === 'help'" class="popover-menu popover-right" style="width: 450px;">
            <ol style="margin: 0; padding-left: 15px; line-height: 1.6; font-size: 13px;">
              <li style="margin-bottom: 10px;" v-html="t('help_editor_1')"></li>
              <li style="margin-bottom: 10px;" v-html="t('help_editor_2')"></li>
              <li style="margin-bottom: 10px;" v-html="t('help_editor_3')"></li>
              <li style="margin-bottom: 10px;" v-html="t('help_editor_4')"></li>
              <li style="margin-bottom: 10px;" v-html="t('help_editor_5')"></li>
              <li style="margin-bottom: 10px;" v-html="t('help_editor_6')"></li>
              <li v-html="t('help_editor_7')"></li>
            </ol>
          </div>
        </div>

        <div style="display: flex; align-items: center; gap: 15px; margin-left: 5px;" v-if="!isEditorLoading">
          <span v-if="editorDirty" class="save-state save-dirty">● {{ t('unsaved_changes') }}</span>
          <span v-else-if="lastSavedAt" class="save-state save-saved">{{ t('saved_at') }} {{ lastSavedAt }}</span>
          <button v-if="parsedBlocks.length > 0" class="btn btn-primary" @click="saveFile">{{ t('save') }}</button>
        </div>
      </template>
    </div>
    <Teleport to="body">
      <div v-if="eggDrops.length" class="egg-rain">
        <img v-for="d in eggDrops" :key="d.id" :src="appLogo" class="egg-drop"
             :style="{ left: d.left + '%', width: d.size + 'px', animationDelay: d.delay + 's', animationDuration: d.dur + 's', '--rot': d.rot + 'deg' }" alt="" />
      </div>
    </Teleport>
  </header>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open, ask } from '@tauri-apps/plugin-dialog';
import { t, SCRIPT_CODES } from '../locales.js';
import Icon from './Icon.vue';
import AccentPicker from './AccentPicker.vue';
import appLogo from '../assets/app-logo.png';
import { 
  uiTheme, uiLang, targetLang, targetScript, sourceLang, currentMode, activePopover, isAiModalOpen,
  parsedBlocks, isEditorLoading, currentFilePath, showSourceModal, showAddStringModal, MANUAL_FILE, showAboutModal,
  projectPath, hiddenFiles, isProcessing, loadProjectSettings, getFileName, showMsg, showHidden,
  availableLanguages, editorDirty, lastSavedAt, uiAccent, FUNNY_PROMPTS, scrollToBlock, editorResizeTick
} from '../store.js';
import { refreshProject, exportCSV, exportJSON, importCSV, importJSON, exportPO, importPO, saveFile, getBlockStatus } from '../actions.js';
import { fixFile, hasBulkFixables, diagnose } from '../diagnostics.js';

const predefinedLangs =['russian', 'english', 'spanish', 'french', 'german'];
const targetLangSelect = ref(targetLang.value === '' ? '' : (predefinedLangs.includes(targetLang.value) ? targetLang.value : 'custom'));
// Синхронизируем выпадашку при смене проекта (loadProjectSettings меняет targetLang извне)
watch(targetLang, (v) => {
  const desired = v === '' ? '' : (predefinedLangs.includes(v) ? v : 'custom');
  if (targetLangSelect.value !== desired) targetLangSelect.value = desired;
});

const hasErrors = computed(() => parsedBlocks.value.some(block => getBlockStatus(block) === 'error'));
const reviewCount = computed(() => parsedBlocks.value.filter(block => getBlockStatus(block) === 'outdated').length);
const hasReview = computed(() => reviewCount.value > 0);
const hasFixables = computed(() => hasBulkFixables(parsedBlocks.value));
// Состояние индикатора «Проверка»: ошибки -> !, предупреждения (к проверке/UI-варнинги) -> ?, иначе -> ✓.
const hasWarnDiag = computed(() => parsedBlocks.value.some(b => diagnose(b).some(d => d.severity === 'warning')));
const hasWarnings = computed(() => hasReview.value || hasWarnDiag.value);
const qaState = computed(() => hasErrors.value ? 'error' : (hasWarnings.value ? 'warn' : 'ok'));
const qaIcon = computed(() => hasErrors.value ? 'alert' : (hasWarnings.value ? 'help' : 'check'));

function jumpToNextWarning() {
  const b = parsedBlocks.value.find(x => diagnose(x).some(d => d.severity === 'warning'));
  if (b) scrollToBlock(b.id);
}

// Массовая починка файла: безопасные (bulk) автофиксы по всем строкам — срезает прилипшие
// префиксы-эхо и восстанавливает потерянные ведущие токены. Перенос UI (субъективный) — не в массовой.
function fixFileAll() {
  const n = fixFile(parsedBlocks.value);
  if (n > 0) { editorDirty.value = true; editorResizeTick.value++; }
  showMsg('success', t('diag_fixed_n').replace('{n}', n));
}
let reviewIdx = 0;

function togglePopover(name) { activePopover.value = activePopover.value === name ? null : name; }

function saveSettings() {
  localStorage.setItem('renforge_ui_lang', uiLang.value);
  localStorage.setItem('renforge_target_lang', targetLang.value);
  localStorage.setItem('renforge_source_lang', sourceLang.value);
  localStorage.setItem('renforge_ui_theme', uiTheme.value);
  localStorage.setItem('renforge_target_script', targetScript.value);
  localStorage.setItem('renforge_ui_accent', uiAccent.value);
}

// --- Пасхалка: 30 кликов подряд по лого ---
const eggDrops = ref([]);
let logoClicks = 0;
let lastLogoClickTs = 0;
let lastFunnyIdx = -1;
function onLogoClick() {
  const now = Date.now();
  if (now - lastLogoClickTs > 2000) logoClicks = 0; // не подряд — сброс
  lastLogoClickTs = now;
  logoClicks++;
  if (logoClicks >= 30) { logoClicks = 0; triggerEasterEgg(); }
}
function triggerEasterEgg() {
  // 1) Кувырок всего окна
  const app = document.querySelector('.app-container');
  if (app) { app.classList.add('rf-barrel'); setTimeout(() => app.classList.remove('rf-barrel'), 1200); }
  // 2) Ливень из мини-девочек
  const drops = [];
  for (let i = 0; i < 30; i++) {
    drops.push({
      id: i + '-' + Date.now(),
      left: Math.random() * 100,
      size: 24 + Math.round(Math.random() * 40),
      delay: +(Math.random() * 0.9).toFixed(2),
      dur: +(2.4 + Math.random() * 2.2).toFixed(2),
      rot: Math.round(Math.random() * 1080 - 540),
    });
  }
  eggDrops.value = drops;
  setTimeout(() => { eggDrops.value = []; }, 6500);
  // 3) Тихо подменяем системный промпт нейросети на случайный смешной
  //    (не повторяем предыдущий, чтобы каждый раз было что-то новое).
  //    Чинится кнопкой «Сбросить к стандарту» в AI-ассистенте.
  let idx = Math.floor(Math.random() * FUNNY_PROMPTS.length);
  if (FUNNY_PROMPTS.length > 1) {
    while (idx === lastFunnyIdx) idx = Math.floor(Math.random() * FUNNY_PROMPTS.length);
  }
  lastFunnyIdx = idx;
  localStorage.setItem('renforge_ollama_system', FUNNY_PROMPTS[idx]);
  // 4) Ехидный тост
  showMsg('success', t('egg_msg'), 5000);
}

function onTargetLangSelect() {
  if (targetLangSelect.value !== 'custom') targetLang.value = targetLangSelect.value;
  else if (predefinedLangs.includes(targetLang.value)) targetLang.value = '';
  saveSettings();
  if (projectPath.value) refreshProject();
}

async function openProjectFolder() {
  try {
    const selectedPath = await open({ multiple: false, directory: true });
    if (!selectedPath) return; 
    projectPath.value = selectedPath; 
    loadProjectSettings(); 
    await refreshProject();
  } catch (e) { showMsg('error', `Error: ${e}`); }
}

async function closeEditor() {
    if (editorDirty.value) {
        const ok = await ask(t('confirm_leave_unsaved'), { title: t('unsaved_changes'), kind: 'warning' });
        if (!ok) return;
    }
    currentMode.value = 'dashboard';
    currentFilePath.value = '';
    parsedBlocks.value =[];
    editorDirty.value = false;
}

function jumpToNextError() {
    const errBlock = parsedBlocks.value.find(b => getBlockStatus(b) === 'error');
    if (errBlock) scrollToBlock(errBlock.id);
}

function jumpToNextReview() {
    const blocks = parsedBlocks.value.filter(b => getBlockStatus(b) === 'outdated');
    if (!blocks.length) return;
    if (reviewIdx >= blocks.length) reviewIdx = 0;
    const b = blocks[reviewIdx];
    reviewIdx++;
    scrollToBlock(b.id);
}
</script>