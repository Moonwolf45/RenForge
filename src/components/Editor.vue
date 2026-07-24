<template>
  <div class="workspace">
    <div v-if="isEditorLoading" class="editor-loading">
      <div class="spinner"></div>
      <p style="font-weight: 600;">{{ t('loading_editor') }}</p>
    </div>

    <template v-else>
      <div v-if="parsedBlocks.length === 0" class="raw-preview">
        <EmptyState v-if="currentFilePath === MANUAL_FILE" icon="plus" :title="t('manual_empty_title')" :hint="t('manual_empty_hint')" />
        <EmptyState v-else icon="file" :title="t('raw_no_blocks')" />
      </div>

      <template v-else>
        <!-- SIDEBAR -->
        <aside class="sidebar">
          <div class="sidebar-title">{{ t('file_structure') }}</div>
          <div class="editor-progress">
            <div class="ep-row">
              <span>{{ t('stat_translated') }}</span>
              <span class="ep-count">{{ doneCount }} / {{ totalCount }}</span>
            </div>
            <div class="progress-bar-bg"><div class="progress-bar-fill" :style="{ width: pct + '%' }"></div></div>
          </div>
          <div style="padding: 10px 15px; border-bottom: 1px solid var(--border-main);">
            <div class="editor-search-wrap">
              <span class="editor-search-icon"><Icon name="search" :size="14" /></span>
              <input type="text" v-model="editorSearch" :placeholder="t('editor_search_ph')" class="editor-search" />
              <button v-if="editorSearch" class="editor-search-clear" @click="editorSearch = ''" :title="t('close')"><Icon name="x" :size="13" /></button>
            </div>
            <div v-if="editorSearch.trim()" class="editor-search-count">{{ matchCount }} {{ t('strings_word') }}</div>
            <label class="toggle-hidden" style="margin: 8px 0 0;">
              <input type="checkbox" v-model="hideTranslated" />
              {{ t('hide_translated') }}
            </label>
          </div>
          <div class="sidebar-list">
            <template v-for="(block, index) in parsedBlocks" :key="'nav-' + block.id">
              <div class="sidebar-item" 
                   @click="focusBlockByIndex(index)"
                   v-show="blockVisible(block)">
                <span class="status-dot" :class="getBlockStatus(block)"></span>
                <span class="sidebar-index">{{ index + 1 }}</span>
                <span class="sidebar-id">{{ block.id }}</span>
              </div>
            </template>
          </div>
        </aside>

        <!-- MAIN EDITOR -->
        <main class="editor-panel">
          <div class="live-reload-hint">
            <Icon name="info" :size="16" />
            <span>{{ t('live_reload_hint') }}</span>
          </div>
          <div class="kbd-hint">{{ t('kbd_hint') }}</div>

          <div class="translation-block" 
               v-for="(block, index) in parsedBlocks" 
               :key="block.id" 
               :id="'block-' + block.id" 
               :class="['status-' + getBlockStatus(block), { 'row-flash': flashBlockId === block.id }]"
               v-show="blockVisible(block)">
            <div class="block-header">
              <div class="block-id-group">
                <span class="block-id">#{{ index + 1 }}<span v-if="block.line_number"> · {{ t('line_num') }} {{ block.line_number }}</span> | ID: {{ block.id }}</span>
                <span v-if="block.source" class="src-tag" :class="'src-' + block.source"
                      :title="block.source === 'ast' ? t('src_ast_hint') : t('src_regex_hint')">{{ block.source === 'ast' ? 'AST' : 'Regex' }}</span>
                <span v-if="dupInfo(block)" class="dup-badge" :class="{ 'dup-badge-conflict': dupInfo(block).variants > 1 }" :title="dupTitle(block)">
                  <Icon name="copy" :size="11" />{{ dupInfo(block).count }}<template v-if="dupInfo(block).variants > 1">!</template>
                </span>
              </div>
              <div class="card-actions block-actions">
                <select class="block-channel" :class="{ 'channel-set': block.channel }" :value="block.channel || 'auto'" @change="setChannel(block, $event.target.value)" :title="t('channel_hint')">
                  <option value="auto">{{ t('channel_auto') }}</option>
                  <option value="say">{{ t('channel_say') }}</option>
                  <option value="ui">{{ t('channel_ui') }}</option>
                  <option value="both">{{ t('channel_both') }}</option>
                </select>
                <button v-if="isManualString(block)" class="icon-text-btn" @click="manualEditTarget = block; showAddStringModal = true" :title="t('edit_string')"><Icon name="edit" :size="15" /></button>
                <button v-if="isManualString(block)" class="icon-text-btn icon-danger" @click="deleteManualString(block)" :title="t('manual_delete')"><Icon name="trash" :size="15" /></button>
                <button class="icon-text-btn" :class="{ 'confirm-on': block.confirmed }" @click="toggleConfirmed(block)" :title="block.confirmed ? t('confirm_off') : t('confirm_on')"><Icon name="check" :size="15" /></button>
                <button class="icon-text-btn" @click="copyToClipboard(block)" :title="t('copy_original')"><Icon name="copy" :size="15" /></button>
                <button class="icon-text-btn" @click="copyOriginal(block)" :title="t('copy_paste_original')"><Icon name="arrow-down" :size="15" /></button>
                <button class="icon-text-btn" @click="clearTranslation(block)" :title="t('clear_field')"><Icon name="x" :size="15" /></button>
              </div>
            </div>
            
            <div class="original-text" @click="onOriginalClick(block, $event)">
              <span v-if="block.who" class="char-prefix original-prefix">
                <span class="char-mapping-name" v-if="charMap[block.who.trim()]">
                  {{ charMap[block.who.trim()] }}
                </span>
                <span v-else>{{ block.who.trim() }}</span>
                <span v-if="charMap[block.who.trim()]" class="char-raw">({{ block.who.trim() }})</span>
              </span>
              <span class="original-body" v-html="hl(block)"></span>
            </div>

            <!-- Альт-варианты (multi-key): иные формулировки того же текста в языке-источнике.
                 Перевод один — доставится под всеми вариантами. Здесь только контекст. -->
            <div v-if="altTexts(block).length" class="alt-variants" :title="t('alt_variant_hint')">
              <span class="alt-variants-label">{{ t('alt_variant_label') }}:</span>
              <span v-for="(a, ai) in altTexts(block)" :key="ai" class="alt-variant">{{ a }}</span>
            </div>

            <!-- Конфликт дубликатов (#3): один оригинал переведён по-разному → в игру уйдёт один -->
            <div v-if="dupInfo(block) && dupInfo(block).variants > 1" class="dup-conflict-warn">
              <Icon name="alert" :size="14" />
              <span>{{ t('dup_conflict').replace('{n}', dupInfo(block).variants) }}</span>
            </div>

            <!-- Перенесено из прошлой версии: показываем прежний оригинал -->
            <div v-if="getBlockStatus(block) === 'outdated'" class="outdated-diff">
              <span class="outdated-badge">{{ t('status_outdated') }}</span>
              <span v-if="block.prev_original === block.original" class="outdated-prev tm-fill-note">{{ t('tm_from_memory') }}</span>
              <span v-else class="outdated-prev"><b>{{ t('was_label') }}:</b> {{ block.prev_original }}</span>
              <button class="icon-text-btn" @click="resolveOutdated(block)" :title="t('mark_reviewed')"><Icon name="check" :size="15" /></button>
            </div>

            <!-- ТЕГИ: клик вставляет тег в перевод; отсутствующие подсвечены -->
            <div class="tag-chips" v-if="getOriginalTags(block).length > 0">
              <span class="tag-chips-label">{{ t('tags_label') }}:</span>
              <button v-for="(tag, ti) in getOriginalTags(block)" :key="ti"
                      class="tag-chip" :class="{ 'tag-needed': !(block.translation || '').includes(tag) }"
                      @click="insertAtCaret(block, tag)" :title="t('insert_tag')">{{ tag }}</button>
            </div>

            <div class="fake-input-wrapper">
              <div v-if="block.who" class="char-prefix translated-prefix">{{ block.who.trim() }}</div>
              <textarea class="transparent-input" 
                     rows="1"
                     :id="'ta-' + block.id"
                     v-autogrow
                     v-model="block.translation" 
                     :placeholder="t('input_placeholder')"
                     @input="onInput(block, $event)"
                     @keydown="onKeydown($event, index)"
                     @focus="focusedBlockId = block.id"
                     @blur="focusedBlockId = null"></textarea>
            </div>
            <div class="diagnostics" v-if="diagnose(block).length > 0">
              <div class="diag" :class="'diag-' + d.severity" v-for="d in diagnose(block)" :key="d.id">
                <strong>{{ t(d.msgKey) }}</strong>
                <span class="missing-tag" v-for="(it, i) in d.items" :key="i">{{ it }}</span>
                <button v-if="d.fixable" class="btn btn-outline diag-fix" @click="fix(block, d.id)">{{ t('diag_fix') }}</button>
              </div>
            </div>
          </div>
        </main>

        <!-- GLOSSARY SIDEBAR -->
        <aside class="assistant-sidebar" :class="{ collapsed: !glossaryOpen }">
          <div class="sidebar-title glossary-title" :class="{ clickable: !glossaryOpen }" @click="expandGlossary">
            <span>{{ t('glossary') }}</span>
            <button v-if="glossaryOpen" class="icon-text-btn glossary-collapse-btn" @click.stop="glossaryOpen = false" :title="t('close')"><Icon name="x" :size="14" /></button>
            <Icon v-else name="plus" :size="14" class="glossary-expand-ic" />
          </div>
          <div class="glossary-content" v-show="glossaryOpen">
            <div class="glossary-add-form">
              <input type="text" v-model="newTerm.original" :placeholder="t('glos_orig')" />
              <input type="text" v-model="newTerm.translation" :placeholder="t('glos_tran')" @keyup.enter="addGlossaryTerm" />
              <button class="btn btn-primary" @click="addGlossaryTerm">{{ t('glos_add') }}</button>
            </div>
            <p class="glossary-hint">{{ t('glossary_click_hint') }}</p>
            
            <div class="glossary-list">
              <div class="glossary-card" v-for="(term, i) in glossary" :key="i">
                <div class="glos-terms">
                  <div class="glos-original" :title="term.original">{{ term.original }}</div>
                  <div class="glos-translation" :title="term.translation">{{ term.translation }}</div>
                </div>
                <button class="glos-del-btn" @click="removeGlossaryTerm(i)" :title="t('clear_field')"><Icon name="trash" :size="14" /></button>
              </div>
            </div>
          </div>
        </aside>
      </template>
    </template>
  </div>
</template>

<script setup>
import { ref, computed, nextTick, watch, onUnmounted } from 'vue';
import { 
    isEditorLoading, parsedBlocks, hideTranslated, 
    focusedBlockId, charMap, dupMap, glossary, newTerm, editorDirty, showMsg, editorResizeTick,
    flashBlockId, flashBlock,
    currentFilePath, MANUAL_FILE, showAddStringModal, manualEditTarget
} from '../store.js';
import { getBlockStatus, getOriginalTags, isManualString, deleteManualString } from '../actions.js';
import { diagnose, applyFix, clearDiagnostics } from '../diagnostics.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';
import EmptyState from './EmptyState.vue';

const glossaryOpen = ref(true);

// Поиск по строкам файла (фильтрует и сайдбар, и основной список). Ищет по тексту
// оригинала/перевода, ID строки и имени говорящего — без учёта регистра.
const editorSearch = ref('');
// Альт-варианты строки (multi-key): иные формулировки того же текста в языке-источнике
// (напр. base + tl/english). alt_texts в БД — JSON-массив; парсим для показа контекста.
function altTexts(block) {
  const raw = block && block.alt_texts;
  if (!raw) return [];
  try { const arr = JSON.parse(raw); return Array.isArray(arr) ? arr : []; }
  catch (e) { return []; }
}
// Инфо о дубликатах строки (из dupMap): {count, variants} или null.
// count>1 — тот же оригинал есть ещё в проекте (перевод общий, доставится один вариант);
// variants>1 — переведён по-разному → в игру уйдёт ОДИН вариант (изъян #3, конфликт).
function dupInfo(block) {
  const o = block && block.original;
  return (o && dupMap.value[o]) || null;
}
function dupTitle(block) {
  const d = dupInfo(block);
  if (!d) return '';
  return d.variants > 1
    ? t('dup_conflict').replace('{n}', d.variants)
    : t('dup_hint').replace('{n}', d.count);
}
// Переопределение канала доставки строки: 'auto' (по типу) | 'say' | 'ui' | 'both'.
function setChannel(block, val) {
  block.channel = (val === 'auto') ? null : val;
  editorDirty.value = true;
}
function blockVisible(block) {
  // фильтр «скрыть переведённые» (текущий редактируемый блок не прячем; строки с любой
  // диагностикой — предупреждение/ошибка — тоже не прячем, иначе их не видно и не достичь
  // навигацией «Предупреждения», хотя значок в шапке горит).
  if (hideTranslated.value && getBlockStatus(block) === 'translated'
      && focusedBlockId.value !== block.id && diagnose(block).length === 0) return false;
  const q = editorSearch.value.trim().toLowerCase();
  if (q) {
    const hay = `${block.id}\n${block.original || ''}\n${block.translation || ''}\n${block.who || ''}`.toLowerCase();
    if (!hay.includes(q)) return false;
  }
  return true;
}
const matchCount = computed(() =>
  editorSearch.value.trim() ? parsedBlocks.value.filter(blockVisible).length : 0
);

const totalCount = computed(() => parsedBlocks.value.length);
const doneCount = computed(() => parsedBlocks.value.filter(b => getBlockStatus(b) === 'translated').length);
const pct = computed(() => totalCount.value ? Math.round((doneCount.value / totalCount.value) * 100) : 0);

// Авто-рост textarea по содержимому
function resize(el) {
  if (!el) return;
  el.style.height = 'auto';
  el.style.height = el.scrollHeight + 'px';
}

// Ленивый autogrow: на больших файлах синхронный замер scrollHeight у тысяч textarea
// при монтировании вызывал шторм reflow (главная причина «долгого открытия»).
// Теперь подгоняем высоту только когда textarea реально попадает в видимую область
// (IntersectionObserver), а после первого замера перестаём наблюдать. Видимые на
// старте блоки подгоняются сразу, остальные — по мере прокрутки.
const autogrowObserver = typeof IntersectionObserver !== 'undefined'
  ? new IntersectionObserver((entries, obs) => {
      for (const e of entries) {
        if (e.isIntersecting) { resize(e.target); obs.unobserve(e.target); }
      }
    }, { rootMargin: '300px 0px' })
  : null;

const vAutogrow = {
  mounted(el) {
    if (autogrowObserver) autogrowObserver.observe(el);
    else nextTick(() => resize(el));
  },
  unmounted(el) { if (autogrowObserver) autogrowObserver.unobserve(el); },
};

// Мемоизация подсветки глоссария: highlightGlossary вызывался для КАЖДОГО блока на
// КАЖДЫЙ ререндер (а ререндер случается при каждом нажатии клавиши). Кэшируем результат
// по id блока; сбрасываем кэш при смене глоссария или открытии другого файла.
const hlCache = new Map();
function hl(block) {
  let v = hlCache.get(block.id);
  if (v === undefined) { v = highlightGlossary(block.original); hlCache.set(block.id, v); }
  return v;
}
watch(glossary, () => hlCache.clear(), { deep: true });
watch(parsedBlocks, () => { hlCache.clear(); clearDiagnostics(); });

// После пакетного перевода (AI/импорт заполняет многострочные переводы) подгоняем
// высоту всех видимых textarea — иначе они остаются в одну строку до фокуса.
watch(editorResizeTick, () => {
  nextTick(() => {
    document.querySelectorAll('.editor-panel .transparent-input').forEach(resize);
  });
});

onUnmounted(() => { if (autogrowObserver) autogrowObserver.disconnect(); });

function onInput(block, e) {
  editorDirty.value = true;
  if (block.prev_original) block.prev_original = ''; // правка снимает пометку «требует проверки»
  resize(e.target);
}

function resolveOutdated(block) {
  block.prev_original = '';
  editorDirty.value = true;
}

// Ручная отметка «перевод подтверждён» (для строк, где перевод совпадает с оригиналом).
// Подтверждение пустой строки трактуем как «оригинал и есть перевод» — вписываем оригинал
// (станет translation===original; при доставке такая пара пропускается как no-op).
function toggleConfirmed(block) {
  block.confirmed = !block.confirmed;
  if (block.confirmed && !(block.translation && block.translation.trim()) && (block.original || '').trim()) {
    block.translation = block.original;
    editorResizeTick.value++;
    nextTick(() => resize(document.getElementById('ta-' + block.id)));
  }
  editorDirty.value = true;
}

// Применить автофикс одной диагностики к блоку (кнопка «Исправить» у проблемы).
function fix(block, ruleId) {
  const res = applyFix(block, ruleId);
  if (res != null && res !== block.translation) {
    block.translation = res;
    editorDirty.value = true;
    editorResizeTick.value++;
    nextTick(() => resize(document.getElementById('ta-' + block.id)));
  }
}

// --- Навигация с клавиатуры ---
function nextIndex(from, untranslatedOnly) {
  for (let i = from + 1; i < parsedBlocks.value.length; i++) {
    const b = parsedBlocks.value[i];
    if (hideTranslated.value && getBlockStatus(b) === 'translated') continue;
    if (untranslatedOnly && getBlockStatus(b) === 'translated') continue;
    return i;
  }
  return -1;
}

function focusBlockByIndex(i) {
  if (i < 0 || i >= parsedBlocks.value.length) return;
  const b = parsedBlocks.value[i];
  // Ленивый авторост соседних полей во время скролла смещает цель — центрирование
  // считалось по «сжатой» раскладке и промахивалось (прыжок на другую строку, верно
  // только со второго раза). Перед навигацией доводим высоту всех видимых textarea до
  // финальной, чтобы оффсет цели был окончательным, и лишь затем центрируем.
  document.querySelectorAll('.editor-panel .transparent-input').forEach(resize);
  nextTick(() => {
    const el = document.getElementById('ta-' + b.id);
    if (el) { el.focus({ preventScroll: true }); el.scrollIntoView({ behavior: 'instant', block: 'center' }); }
    flashBlock(b.id);
  });
}

function onKeydown(e, index) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    const i = nextIndex(index, e.ctrlKey || e.metaKey);
    if (i >= 0) focusBlockByIndex(i);
    else e.target.blur();
  } else if (e.key === 'Escape') {
    e.target.blur();
  }
}

// --- Быстрые действия ---
function copyOriginal(block) {
  block.translation = block.original;
  editorDirty.value = true;
  nextTick(() => resize(document.getElementById('ta-' + block.id)));
}
async function copyToClipboard(block) {
  try {
    await navigator.clipboard.writeText(block.original || '');
    showMsg('success', t('copied_clipboard'), 2000);
  } catch (e) {
    showMsg('error', e.toString());
  }
}
function clearTranslation(block) {
  block.translation = '';
  editorDirty.value = true;
  nextTick(() => resize(document.getElementById('ta-' + block.id)));
}

// --- Вставка тега/термина в позицию курсора ---
function insertAtCaret(block, text) {
  const el = document.getElementById('ta-' + block.id);
  const cur = block.translation || '';
  let start = cur.length, end = cur.length;
  if (el && typeof el.selectionStart === 'number') { start = el.selectionStart; end = el.selectionEnd; }
  block.translation = cur.slice(0, start) + text + cur.slice(end);
  editorDirty.value = true;
  nextTick(() => {
    if (el) {
      el.focus();
      const pos = start + text.length;
      el.setSelectionRange(pos, pos);
      resize(el);
    }
  });
}

function onOriginalClick(block, e) {
  const w = e.target.closest ? e.target.closest('.glossary-word') : null;
  if (w && w.dataset.tr) insertAtCaret(block, w.dataset.tr);
}

// --- Глоссарий ---
function addGlossaryTerm() {
    if (!newTerm.value.original.trim() || !newTerm.value.translation.trim()) return;
    glossary.value.push({ original: newTerm.value.original.trim(), translation: newTerm.value.translation.trim() });
    newTerm.value = { original: '', translation: '' };
}
function removeGlossaryTerm(index) { glossary.value.splice(index, 1); }

// Разворачиваем глоссарий кликом по свёрнутой полосе. В развёрнутом виде клик по шапке
// НЕ сворачивает — для этого есть отдельная кнопка-крестик (чтобы не закрывалось случайно).
function expandGlossary() { if (!glossaryOpen.value) glossaryOpen.value = true; }

function escapeHtml(unsafe) {
    if (!unsafe) return '';
    return unsafe.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#039;");
}

function highlightGlossary(text) {
    if (!text) return '';
    let res = escapeHtml(text);
    const sortedTerms = [...glossary.value].sort((a, b) => b.original.length - a.original.length);
    for (const term of sortedTerms) {
        if (!term.original) continue;
        const escapedTerm = escapeHtml(term.original).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const regex = new RegExp(`(${escapedTerm})`, 'gi');
        res = res.replace(regex, `<span class="glossary-word" data-tr="${escapeHtml(term.translation)}" title="${escapeHtml(term.translation)}">$1</span>`);
    }
    return res;
}
</script>
