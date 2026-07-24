<template>
  <div class="modal-overlay" @click.self="close">
    <div class="modal-content source-modal">
      <div class="modal-header">
        <h2><Icon name="file" :size="16" /> {{ getFileName(currentFilePath) }}</h2>
        <div class="source-head-right">
          <div class="segmented-control source-tabs">
            <button :class="['seg-btn', { active: tab === 'original' }]" @click="setTab('original')">{{ t('source_tab_original') }}</button>
            <button :class="['seg-btn', { active: tab === 'renforge' }]" @click="setTab('renforge')">{{ t('source_tab_renforge') }}</button>
          </div>
          <button class="icon-close-btn" @click="close" :title="t('close')"><Icon name="x" :size="18" /></button>
        </div>
      </div>

      <div class="source-main">
        <div v-if="loading" class="source-status"><span class="src-spinner"></span> {{ t('source_decompiling') }}</div>
        <div v-else-if="error" class="source-status source-error"><Icon name="info" :size="18" /> {{ error }}</div>

        <template v-else>
          <div class="source-scroll" ref="scrollEl" @scroll="onScroll">
            <div class="source-code">
              <div
                v-for="(r, i) in renderedLines"
                :key="i"
                :data-ln="i + 1"
                class="src-line"
                :class="{
                  'src-hl': tab === 'original' && showExtracted && extractedLines.has(i + 1),
                  'src-cand': showCandidates && tab === 'original' && !extractedLines.has(i + 1) && candidateMap.has(i + 1)
                }"
              >
                <span class="src-gutter">{{ i + 1 }}</span>
                <span v-if="r.levels" class="src-indents">
                  <span v-for="g in r.levels" :key="g" class="indent-guide"></span>
                </span>
                <code class="src-text"><span v-for="(tk, ti) in r.tokens" :key="ti" :class="'tok-' + tk.t">{{ tk.v }}</span><span v-if="!r.tokens.length"> </span></code>
                <button
                  v-if="showCandidates && tab === 'original' && !extractedLines.has(i + 1) && candidateMap.has(i + 1)"
                  class="src-cand-add" @click.stop="addCandidate(i + 1)" :title="t('cand_add_hint')"
                ><Icon name="plus" :size="13" /></button>
              </div>
            </div>
          </div>

          <!-- Минимап (как в VS Code): силуэт кода + рамка видимой области -->
          <div class="minimap" ref="miniWrap" @mousedown="onMiniDown">
            <canvas ref="miniCanvas"></canvas>
            <div class="minimap-viewport" :style="{ top: vp.top + 'px', height: vp.height + 'px' }"></div>
          </div>
        </template>
      </div>

      <div class="source-foot">
        <template v-if="tab === 'original'">
          <div class="src-foot-rows">
            <div class="src-foot-row">
              <label class="src-legend src-legend-toggle" :title="t('source_legend')">
                <input type="checkbox" v-model="showExtracted" />
                <span class="src-legend-dot"></span>
                {{ t('source_legend') }}<template v-if="extractedSorted.length"> · {{ extractedSorted.length }}</template>
              </label>
              <div class="src-nav" v-if="showExtracted && extractedSorted.length">
                <button class="btn btn-secondary src-nav-btn" @click="step(-1)" :title="t('prev')"><Icon name="arrow-down" :size="14" /></button>
                <span class="src-nav-pos">{{ navIdx + 1 }} / {{ extractedSorted.length }}</span>
                <button class="btn btn-secondary src-nav-btn src-nav-next" @click="step(1)" :title="t('next')"><Icon name="arrow-down" :size="14" /></button>
              </div>
            </div>
            <div class="src-foot-row">
              <label class="src-legend src-cand-toggle" :title="t('cand_hint')">
                <input type="checkbox" v-model="showCandidates" />
                <span class="src-cand-dot"></span>
                {{ t('cand_legend') }}<template v-if="candidateCount"> · {{ candidateCount }}</template>
              </label>
              <div class="src-cand-actions" v-if="showCandidates && candidateSorted.length">
                <button class="btn btn-secondary src-addall-btn" @click="addAllCandidates" :title="t('cand_add_all_hint')"><Icon name="plus" :size="13" /> {{ t('cand_add_all') }} · {{ candidateSorted.length }}</button>
                <div class="src-nav src-nav-cand">
                  <button class="btn btn-secondary src-nav-btn" @click="stepCand(-1)" :title="t('prev')"><Icon name="arrow-down" :size="14" /></button>
                  <span class="src-nav-pos">{{ candNavIdx + 1 }} / {{ candidateSorted.length }}</span>
                  <button class="btn btn-secondary src-nav-btn src-nav-next" @click="stepCand(1)" :title="t('next')"><Icon name="arrow-down" :size="14" /></button>
                </div>
              </div>
            </div>
          </div>
        </template>
        <span v-else class="src-legend">{{ t('source_tab_renforge') }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { showSourceModal, currentFilePath, parsedBlocks, projectPath, targetLang, getFileName, showMsg } from '../store.js';
import { addManualString } from '../actions.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const tab = ref('original');
const loadingMap = ref({ original: false, renforge: false });
const errorMap = ref({ original: '', renforge: '' });
const loading = computed(() => loadingMap.value[tab.value]);
const error = computed(() => errorMap.value[tab.value]);
const original = ref(null);
const renforge = ref(null);
const scrollEl = ref(null);
const miniWrap = ref(null);
const miniCanvas = ref(null);
const navIdx = ref(0);
const vp = ref({ top: 0, height: 0 });

const lines = computed(() => {
  const s = tab.value === 'original' ? original.value : renforge.value;
  return s == null ? [] : s.replace(/\r\n/g, '\n').split('\n');
});

// --- Подсветка синтаксиса (лёгкий токенайзер Ren'Py / Python) ---
const KEYWORDS = new Set([
  'label','menu','scene','show','hide','with','jump','call','return','pass','python','init','early',
  'define','default','image','transform','screen','style','translate','voice','play','stop','queue','pause',
  'nvl','window','add','use','vbox','hbox','frame','imagebutton','textbutton','button','bar','vbar','input',
  'key','timer','side','fixed','grid','viewport','imagemap','hotspot','hotbar','at','as','expression','has',
  'if','elif','else','while','for','in','and','or','not','is','None','True','False','del','global','nonlocal',
  'try','except','finally','raise','yield','lambda','import','from','class','def','contains','transclude','on',
  'block','parallel','choice','time','repeat','function','event','text','vpgrid','null','spacing','xalign','yalign',
]);

// Разбивает строку Ren'Py-строки на части: текст / {тег} / [интерполяция].
function splitString(str, out) {
  const re = /(\{[^{}]*\}|\[[^\[\]]*\])/g;
  let last = 0, m;
  while ((m = re.exec(str)) !== null) {
    if (m.index > last) out.push({ t: 'str', v: str.slice(last, m.index) });
    out.push({ t: m[0][0] === '{' ? 'tag' : 'var', v: m[0] });
    last = re.lastIndex;
  }
  if (last < str.length) out.push({ t: 'str', v: str.slice(last) });
}

function tokenize(line) {
  const out = [];
  let i = 0;
  const n = line.length;
  while (i < n) {
    const c = line[i];
    if (c === '#') { out.push({ t: 'com', v: line.slice(i) }); break; }
    if (c === '"' || c === "'") {
      let j = i + 1;
      while (j < n) {
        if (line[j] === '\\') { j += 2; continue; }
        if (line[j] === c) { j++; break; }
        j++;
      }
      out.push({ t: 'str', v: c });
      splitString(line.slice(i + 1, j > i + 1 && line[j - 1] === c ? j - 1 : j), out);
      if (j > i + 1 && line[j - 1] === c) out.push({ t: 'str', v: c });
      i = j;
      continue;
    }
    if (c === ' ' || c === '\t') {
      let j = i + 1;
      while (j < n && (line[j] === ' ' || line[j] === '\t')) j++;
      out.push({ t: 'ws', v: line.slice(i, j) });
      i = j;
      continue;
    }
    if (/[A-Za-z_]/.test(c)) {
      let j = i + 1;
      while (j < n && /[A-Za-z0-9_]/.test(line[j])) j++;
      const word = line.slice(i, j);
      out.push({ t: KEYWORDS.has(word) ? 'kw' : 'id', v: word });
      i = j;
      continue;
    }
    if (/[0-9]/.test(c)) {
      let j = i + 1;
      while (j < n && /[0-9.]/.test(line[j])) j++;
      out.push({ t: 'num', v: line.slice(i, j) });
      i = j;
      continue;
    }
    let j = i + 1;
    while (j < n && /[^\sA-Za-z0-9_"'#]/.test(line[j])) j++;
    out.push({ t: 'op', v: line.slice(i, j) });
    i = j;
  }
  // Имя говорящего: ведущий идентификатор перед строкой (say-стейтмент).
  const firstReal = out.find((tk) => tk.t !== 'ws');
  if (firstReal && firstReal.t === 'id') {
    const after = out.slice(out.indexOf(firstReal) + 1).find((tk) => tk.t !== 'ws');
    if (after && after.t === 'str') firstReal.t = 'fn';
  }
  return out;
}

// Отступы: 4 пробела = уровень (Ren'Py/Python). Направляющие + остаток строки.
const renderedLines = computed(() => lines.value.map((line) => {
  const lead = (line.match(/^ */) || [''])[0].length;
  const levels = Math.floor(lead / 4);
  const rest = line.slice(levels * 4);
  return { levels, rest, tokens: tokenize(rest) };
}));

const extractedLines = computed(() => {
  const set = new Set();
  for (const b of parsedBlocks.value) {
    if (b.line_number) set.add(b.line_number);
  }
  return set;
});
const extractedSorted = computed(() => [...extractedLines.value].sort((a, b) => a - b));
const showExtracted = ref(true);

// --- Кандидаты «возможно пропущено» (эвристика, Feature 3) ---
// Флагуем только высокосигнальные случаи (_(), say, screen-текст) с фильтром «похоже на
// текст», чтобы не зашуметь путями/стилями/ассетами. Это помощник, не второй экстрактор.
const showCandidates = ref(true);
const NONTEXT_KW = new Set(['image','scene','show','hide','play','stop','queue','define','default',
  'transform','style','init','screen','label','jump','call','return','window','camera','voice',
  'python','add','use','at','with','pause','nvl','key','timer']);

function looksTexty(s) {
  const t = (s || '').trim();
  if (!t) return false;
  if (/^[#@$]/.test(t)) return false;                 // цвет / at-выражение
  if (/[\/\\]/.test(t)) return false;                  // пути
  if (/\.(png|jpe?g|webp|gif|bmp|svg|ogg|mp3|wav|opus|flac|ttf|otf|rpyc?|avi|webm|mp4|json)$/i.test(t)) return false;
  if (/^[a-z0-9_]+$/.test(t)) return false;            // голый идентификатор
  if (!/[A-Za-z\u00C0-\uFFFF]/.test(t)) return false;  // нет букв вообще
  return /\s/.test(t) || t.length >= 4;
}

function detectCandidate(rawLine) {
  const s = rawLine.trim();
  if (!s || s[0] === '#') return null;
  let m = s.match(/_\(\s*(["'])((?:[^"'\\]|\\.)*)\1\s*\)/);
  if (m && looksTexty(m[2])) return { text: m[2], kind: 'ui' };
  m = s.match(/^([A-Za-z_]\w*\s+)?(["'])((?:[^\\]|\\.)*?)\2\s*(?:#.*)?$/);
  if (m) {
    const lead = (m[1] || '').trim();
    if ((!lead || !NONTEXT_KW.has(lead)) && looksTexty(m[3])) return { text: m[3], kind: 'dialogue' };
  }
  m = s.match(/^(text|textbutton|tooltip|label)\s+(["'])((?:[^"'\\]|\\.)*)\2/);
  if (m && looksTexty(m[3])) return { text: m[3], kind: 'ui' };
  return null;
}

const candidateMap = computed(() => {
  const map = new Map();
  if (tab.value !== 'original') return map;
  const ls = lines.value;
  for (let i = 0; i < ls.length; i++) {
    const ln = i + 1;
    if (extractedLines.value.has(ln)) continue;
    const c = detectCandidate(ls[i]);
    if (c) map.set(ln, c);
  }
  return map;
});
const candidateCount = computed(() => candidateMap.value.size);
const candidateSorted = computed(() => [...candidateMap.value.keys()].sort((a, b) => a - b));
const candNavIdx = ref(0);
function stepCand(dir) {
  const arr = candidateSorted.value;
  if (!arr.length) return;
  candNavIdx.value = (candNavIdx.value + dir + arr.length) % arr.length;
  jumpToLine(arr[candNavIdx.value]);
}

async function addCandidate(ln) {
  const c = candidateMap.value.get(ln);
  if (!c) return;
  await addManualString(c.text, '', c.kind, true, ln);
  showMsg('success', t('cand_added'));
}

// Массово добавить всех кандидатов файла (восстановление пропущенного экстрактором файла).
// Снимок строк делаем заранее: addManualString вплетает блоки в parsedBlocks, и candidateMap
// по ходу пересчитывается — итерируем по копии, чтобы не потерять записи.
async function addAllCandidates() {
  const items = candidateSorted.value
    .map((ln) => ({ ln, c: candidateMap.value.get(ln) }))
    .filter((x) => x.c);
  if (!items.length) return;
  for (const { ln, c } of items) {
    await addManualString(c.text, '', c.kind, true, ln);
  }
  showMsg('success', t('cand_added_n').replace('{n}', items.length));
}

// --- Минимап ---
function rowHeight() {
  const wrap = miniWrap.value;
  const total = renderedLines.value.length || 1;
  const H = wrap ? wrap.clientHeight : 1;
  return Math.min(4, H / total);
}

function drawMinimap() {
  const cv = miniCanvas.value, wrap = miniWrap.value;
  if (!cv || !wrap) return;
  const W = wrap.clientWidth, H = wrap.clientHeight;
  if (W === 0 || H === 0) return;
  cv.width = W; cv.height = H;
  const ctx = cv.getContext('2d');
  ctx.clearRect(0, 0, W, H);

  const rows = renderedLines.value;
  const total = rows.length || 1;
  const rowH = Math.min(4, H / total);
  const indentUnit = 3, charW = 1.0, pad = 6, markerW = 3;

  const cs = getComputedStyle(cv);
  const v = (name, def) => (cs.getPropertyValue(name) || def).trim();
  const hit = '#52a06b';
  const review = v('--status-review', '#d8b24a');
  const colors = {
    kw: v('--syn-kw', '#569cd6'), str: v('--syn-str', '#ce9178'),
    com: v('--syn-com', '#6a9955'), num: v('--syn-num', '#b5cea8'),
    op: v('--syn-op', '#c0c0c0'), fn: v('--syn-fn', '#dcdcaa'),
    tag: v('--syn-tag', '#c586c0'), var: v('--syn-var', '#4ec9b0'),
    id: v('--text-main', '#cccccc'),
  };

  for (let i = 0; i < total; i++) {
    const r = rows[i];
    const y = i * rowH;
    const hl = tab.value === 'original' && showExtracted.value && extractedLines.value.has(i + 1);
    if (hl) {
      // Маркер извлечённой строки у левого края + лёгкая подложка.
      ctx.globalAlpha = 0.16;
      ctx.fillStyle = hit;
      ctx.fillRect(0, y, W, Math.max(1, rowH - 0.3));
      ctx.globalAlpha = 1;
      ctx.fillStyle = hit;
      ctx.fillRect(0, y, markerW, Math.max(1, rowH - 0.3));
    }
    let x = pad + markerW + r.levels * indentUnit;
    for (const tk of r.tokens) {
      const w = tk.v.length * charW;
      if (tk.t !== 'ws') {
        const cw = Math.min(W - x - pad, w);
        if (cw > 0) {
          ctx.fillStyle = colors[tk.t] || colors.id;
          ctx.globalAlpha = tk.t === 'com' ? 0.55 : 0.85;
          ctx.fillRect(x, y, cw, Math.max(1, rowH - 0.4));
        }
      }
      x += w;
      if (x > W - pad) break;
    }
    // Маркер кандидата «возможно пропущено» — у правого края (амбер).
    if (showCandidates.value && tab.value === 'original' && !extractedLines.value.has(i + 1) && candidateMap.value.has(i + 1)) {
      ctx.globalAlpha = 1;
      ctx.fillStyle = review;
      ctx.fillRect(W - markerW, y, markerW, Math.max(1, rowH - 0.3));
    }
  }
  ctx.globalAlpha = 1;
}

function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  const total = renderedLines.value.length || 1;
  const sh = el.scrollHeight || 1;
  const rowH = rowHeight();
  const firstLine = el.scrollTop * total / sh;
  const visLines = el.clientHeight * total / sh;
  vp.value = { top: firstLine * rowH, height: Math.max(10, visLines * rowH) };
}

function scrollToY(offsetY) {
  const el = scrollEl.value;
  if (!el) return;
  const total = renderedLines.value.length || 1;
  const rowH = rowHeight() || 1;
  const line = offsetY / rowH;
  const lineHeightPx = (el.scrollHeight || 1) / total;
  el.scrollTop = line * lineHeightPx - el.clientHeight / 2;
}

function onMiniDown(e) {
  const wrap = miniWrap.value;
  if (!wrap) return;
  const rect = wrap.getBoundingClientRect();
  const move = (ev) => scrollToY(ev.clientY - rect.top);
  move(e);
  const up = () => {
    window.removeEventListener('mousemove', move);
    window.removeEventListener('mouseup', up);
  };
  window.addEventListener('mousemove', move);
  window.addEventListener('mouseup', up);
}

function jumpToLine(n) {
  const root = scrollEl.value;
  if (!root) return;
  const el = root.querySelector(`[data-ln="${n}"]`);
  if (el) el.scrollIntoView({ block: 'center', behavior: 'smooth' });
}
function step(dir) {
  const arr = extractedSorted.value;
  if (!arr.length) return;
  navIdx.value = (navIdx.value + dir + arr.length) % arr.length;
  jumpToLine(arr[navIdx.value]);
}

async function load(which) {
  loadingMap.value[which] = true; errorMap.value[which] = '';
  try {
    if (which === 'original') {
      original.value = await invoke('decompile_rpyc', { projectPath: projectPath.value, filePath: currentFilePath.value });
    } else {
      renforge.value = await invoke('preview_generated_translations', { projectPath: projectPath.value, targetLang: targetLang.value });
    }
  } catch (e) {
    const s = (e && e.toString) ? e.toString() : String(e);
    if (s.includes('rpyc_missing')) errorMap.value[which] = t('source_no_rpyc');
    else if (s.includes('В базе нет')) errorMap.value[which] = t('source_no_preview');
    else errorMap.value[which] = s;
  } finally {
    loadingMap.value[which] = false;
  }
}

function setTab(name) {
  tab.value = name;
  if (name === 'original' && original.value === null) load('original');
  if (name === 'renforge' && renforge.value === null) load('renforge');
}

function close() { showSourceModal.value = false; }

// Перерисовка минимапа при смене контента/вкладки/извлечённых/кандидатов.
watch([lines, tab, extractedLines, showCandidates, showExtracted], () => {
  nextTick(() => { drawMinimap(); onScroll(); });
});

const onResize = () => { drawMinimap(); onScroll(); };
onMounted(() => window.addEventListener('resize', onResize));
onBeforeUnmount(() => window.removeEventListener('resize', onResize));

load('original');
</script>

<style scoped>
.source-modal { width: 980px; max-width: 95vw; height: 82vh; display: flex; flex-direction: column; padding: 0; }
.source-head-right { display: inline-flex; align-items: center; gap: 12px; }
.source-tabs { margin: 0; }

.source-main { flex: 1; min-height: 0; display: flex; background: var(--bg-base); }
.source-scroll { flex: 1; min-width: 0; overflow: auto; padding: 8px 0; }
.source-code { font-family: ui-monospace, 'Cascadia Code', Consolas, monospace; font-size: 12.5px; min-width: 100%; width: max-content; }

.src-line { display: flex; align-items: stretch; min-height: 20px; line-height: 1.6; border-left: 3px solid transparent; padding-right: 16px; }
.src-line:hover { background: color-mix(in srgb, var(--text-muted) 8%, transparent); }
.src-hl { background: color-mix(in srgb, #52a06b 16%, transparent); border-left-color: #52a06b; }
.src-hl:hover { background: color-mix(in srgb, #52a06b 24%, transparent); }
.src-cand { background: color-mix(in srgb, var(--status-review) 13%, transparent); border-left-color: var(--status-review); }
.src-cand:hover { background: color-mix(in srgb, var(--status-review) 22%, transparent); }
.src-cand-add {
  flex: 0 0 auto; align-self: center; margin-left: 12px;
  width: 22px; height: 22px; display: inline-flex; align-items: center; justify-content: center;
  border: 1px solid var(--status-review); color: var(--status-review); background: transparent;
  border-radius: 5px; cursor: pointer; transition: 0.15s;
}
.src-cand-add:hover { background: var(--status-review); color: var(--bg-base); }
.src-gutter {
  flex: 0 0 52px; text-align: right; padding-right: 14px;
  color: var(--text-muted); user-select: none;
  position: sticky; left: 0; background: var(--bg-base); opacity: 0.85;
  display: flex; align-items: center; justify-content: flex-end;
}
.src-hl .src-gutter { color: #52a06b; opacity: 1; font-weight: 600; }
.src-indents { display: flex; flex: 0 0 auto; align-self: stretch; }
.indent-guide { width: 4ch; border-left: 1px solid color-mix(in srgb, var(--text-muted) 26%, transparent); }
.src-text { flex: 0 0 auto; white-space: pre; color: var(--text-main); background: transparent; align-self: center; }

/* Подсветка синтаксиса */
.tok-kw { color: var(--syn-kw); }
.tok-str { color: var(--syn-str); }
.tok-com { color: var(--syn-com); font-style: italic; }
.tok-num { color: var(--syn-num); }
.tok-op { color: var(--syn-op); }
.tok-fn { color: var(--syn-fn); }
.tok-tag { color: var(--syn-tag); }
.tok-var { color: var(--syn-var); }
.tok-id { color: var(--text-main); }

/* Минимап */
.minimap { position: relative; flex: 0 0 124px; background: color-mix(in srgb, var(--bg-app) 55%, var(--bg-base)); border-left: 1px solid var(--border-main); cursor: pointer; overflow: hidden; }
.minimap canvas { display: block; }
.minimap-viewport {
  position: absolute; left: 0; right: 0;
  background: color-mix(in srgb, var(--text-muted) 20%, transparent);
  border-top: 1px solid color-mix(in srgb, var(--text-muted) 35%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--text-muted) 35%, transparent);
  pointer-events: none;
}

.source-status { flex: 1; display: flex; align-items: center; justify-content: center; gap: 10px; padding: 36px 20px; text-align: center; color: var(--text-muted); }
.source-error { color: var(--error-text); white-space: pre-wrap; }

.source-foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 9px 20px; border-top: 1px solid var(--border-main); background: var(--bg-panel); }
.src-foot-rows { display: flex; flex-direction: column; gap: 7px; flex: 1; min-width: 0; }
.src-foot-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 28px; }
.src-legend { font-size: 12px; color: var(--text-muted); display: inline-flex; align-items: center; gap: 7px; }
.src-legend-toggle, .src-cand-toggle { cursor: pointer; user-select: none; }
.src-legend-toggle input, .src-cand-toggle input { margin: 0; }
.src-legend-dot { width: 12px; height: 12px; border-radius: 3px; background: #52a06b; opacity: 0.8; }
.src-cand-dot { width: 12px; height: 12px; border-radius: 3px; background: var(--status-review); opacity: 0.75; }
.src-nav { display: inline-flex; align-items: center; gap: 8px; }
.src-nav-btn { width: 30px; height: 28px; padding: 0; }
.src-nav-btn :deep(.rf-icon) { transform: rotate(90deg); }
.src-nav-next :deep(.rf-icon) { transform: rotate(-90deg); }
.src-nav-pos { font-size: 12px; color: var(--text-secondary); font-variant-numeric: tabular-nums; min-width: 54px; text-align: center; }
.src-nav-cand .src-nav-pos { color: var(--status-review); }
.src-cand-actions { display: inline-flex; align-items: center; gap: 10px; }
.src-addall-btn { padding: 4px 10px; font-size: 12px; white-space: nowrap; display: inline-flex; align-items: center; gap: 5px; border-color: var(--status-review); color: var(--status-review); }
.src-addall-btn:hover { background: color-mix(in srgb, var(--status-review) 16%, transparent); }

.src-spinner { width: 16px; height: 16px; border: 2px solid color-mix(in srgb, var(--text-muted) 35%, transparent); border-top-color: var(--accent); border-radius: 50%; animation: spin 1s linear infinite; display: inline-block; }
</style>
