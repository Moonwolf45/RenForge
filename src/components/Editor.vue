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
               :class="['status-' + getBlockStatus(block)]"
               v-show="blockVisible(block)">
            <div class="block-header">
              <span class="block-id">#{{ index + 1 }}<span v-if="block.line_number"> · {{ t('line_num') }} {{ block.line_number }}</span> | ID: {{ block.id }}</span>
              <div class="card-actions block-actions">
                <select class="block-channel" :class="{ 'channel-set': block.channel }" :value="block.channel || 'auto'" @change="setChannel(block, $event.target.value)" :title="t('channel_hint')">
                  <option value="auto">{{ t('channel_auto') }}</option>
                  <option value="say">{{ t('channel_say') }}</option>
                  <option value="ui">{{ t('channel_ui') }}</option>
                  <option value="both">{{ t('channel_both') }}</option>
                </select>
                <button v-if="isManualString(block)" class="icon-text-btn" @click="manualEditTarget = block; showAddStringModal = true" :title="t('edit_string')"><Icon name="edit" :size="15" /></button>
                <button v-if="isManualString(block)" class="icon-text-btn icon-danger" @click="deleteManualString(block)" :title="t('manual_delete')"><Icon name="trash" :size="15" /></button>
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
            <div class="tag-error" v-if="getMissingTags(block).length > 0">
              <strong>{{ t('tag_error') }}</strong>
              <span class="missing-tag" v-for="tag in getMissingTags(block)" :key="tag">{{ tag }}</span>
            </div>
            <div class="tag-error" v-if="getExtraInterps(block).length > 0">
              <strong>{{ t('tag_error_extra') }}</strong>
              <span class="missing-tag" v-for="tag in getExtraInterps(block)" :key="tag">{{ tag }}</span>
            </div>
            <div class="ui-length-warn" v-if="uiOverflowWarn(block)">
              <Icon name="info" :size="14" />
              <span>{{ t('ui_length_warn') }}</span>
              <button class="btn btn-outline ui-wrap-btn" @click="wrapToFit(block)">{{ t('ui_wrap_fit') }}</button>
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
    focusedBlockId, charMap, glossary, newTerm, editorDirty, showMsg, editorResizeTick,
    currentFilePath, MANUAL_FILE, showAddStringModal, manualEditTarget
} from '../store.js';
import { getBlockStatus, getMissingTags, getOriginalTags, getExtraInterps, isManualString, deleteManualString } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';
import EmptyState from './EmptyState.vue';

const glossaryOpen = ref(true);

// Поиск по строкам файла (фильтрует и сайдбар, и основной список). Ищет по тексту
// оригинала/перевода, ID строки и имени говорящего — без учёта регистра.
const editorSearch = ref('');
// Переопределение канала доставки строки: 'auto' (по типу) | 'say' | 'ui' | 'both'.
function setChannel(block, val) {
  block.channel = (val === 'auto') ? null : val;
  editorDirty.value = true;
}
function blockVisible(block) {
  // фильтр «скрыть переведённые» (текущий редактируемый блок не прячем)
  if (hideTranslated.value && getBlockStatus(block) === 'translated' && focusedBlockId.value !== block.id) return false;
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
watch(parsedBlocks, () => hlCache.clear());

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

// Предупреждение о длинных UI-переводах: на фиксированном легаси-UI текст без переноса
// и заметно длиннее оригинала может разъехаться (Ren'Py не ужимает текст под область).
// Мягкая некритичная пометка — НЕ блокирует сохранение. Только для UI-строк, без уже
// проставленного переноса, не для совсем коротких подписей.
function uiOverflowWarn(block) {
  if (block.block_type !== 'ui') return false;
  const o = (block.original || '').trim();
  const tr = (block.translation || '').trim();
  if (!tr || tr === o) return false;
  if (tr.includes('\n')) return false; // перенос уже проставлен — переводчик управляет сам
  if (o.length < 6) return false;       // короткие подписи (OK/Да/Меню) не трогаем
  return tr.length > o.length * 1.3 && (tr.length - o.length) >= 4;
}

// Пиксельное измерение видимой ширины строки (теги {..} не видимы — отбрасываем; [var]
// оставляем как есть). Один и тот же шрифт для оригинала и перевода → важна относительная
// ширина, абсолютный кегль сокращается. Фоллбэк на число символов, если canvas недоступен.
const _measureCtx = (() => {
  try { return document.createElement('canvas').getContext('2d'); } catch (e) { return null; }
})();
function visibleWidth(line) {
  const visible = (line || '').replace(/\{[^}]*\}/g, '');
  if (!_measureCtx) return visible.length;
  _measureCtx.font = '20px sans-serif';
  return _measureCtx.measureText(visible).width;
}

// Ассист «Подогнать переносом»: жадно разбиваем перевод на строки в пределах БЮДЖЕТА =
// макс. ширина строки ОРИГИНАЛА (он по замыслу автора вписан в элемент). Переносим только
// по пробелам; теги/[var] не рвём (они внутри неделимых «слов»). Вставляем реальные \n —
// доставка превратит их в перенос строки.
function wrapToFit(block) {
  const origLines = (block.original || '').replace(/\\n/g, '\n').split('\n');
  let budget = 0;
  for (const l of origLines) budget = Math.max(budget, visibleWidth(l));
  if (budget <= 0) return;
  const tr = (block.translation || '').replace(/\\n/g, '\n').replace(/\n/g, ' ').trim();
  if (!tr) return;
  const words = tr.split(/\s+/);
  const lines = [];
  let cur = '';
  for (const wd of words) {
    const cand = cur ? cur + ' ' + wd : wd;
    if (cur && visibleWidth(cand) > budget) {
      lines.push(cur);
      cur = wd;
    } else {
      cur = cand;
    }
  }
  if (cur) lines.push(cur);
  block.translation = lines.join('\n');
  editorDirty.value = true;
  editorResizeTick.value++;
  nextTick(() => resize(document.getElementById('ta-' + block.id)));
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
  const el = document.getElementById('ta-' + b.id);
  if (el) { el.focus(); el.scrollIntoView({ behavior: 'smooth', block: 'center' }); }
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
