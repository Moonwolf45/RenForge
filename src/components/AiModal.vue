<template>
  <div class="modal-overlay" @click.self="isAiModalOpen = false">
      <div class="modal-content ai-modal">
          <div class="modal-header">
              <h2>{{ t('modal_ai_title') }}</h2>
              <button class="icon-close-btn" @click="isAiModalOpen = false" :title="t('close')"><Icon name="x" :size="18" /></button>
          </div>

          <div class="modal-scroll-body">
              <!-- ПЕРЕКЛЮЧАТЕЛЬ РЕЖИМА -->
              <div class="segmented-control ai-mode-switch">
                  <button :class="['seg-btn', { active: aiTab === 'ollama' }]" @click="aiTab = 'ollama'">{{ t('ai_tabs_local') }}</button>
                  <button :class="['seg-btn', { active: aiTab === 'api' }]" @click="aiTab = 'api'">{{ t('ai_tabs_api') }}</button>
                  <button :class="['seg-btn', { active: aiTab === 'manual' }]" @click="aiTab = 'manual'">{{ t('ai_tabs_manual') }}</button>
              </div>

              <!-- ДИАПАЗОН -->
              <div class="ai-range-bar">
                  <div class="ai-range-inputs">
                      <span>{{ t('from') }}</span><input type="number" v-model="aiStart" min="1" :max="parsedBlocks.length" />
                      <span>{{ t('to') }}</span><input type="number" v-model="aiEnd" min="1" :max="parsedBlocks.length" />
                  </div>
                  <div class="ai-presets">
                      <button class="chip" @click="presetAll">{{ t('f_all') }}</button>
                      <button class="chip" @click="presetUntranslated">{{ t('f_todo') }}</button>
                      <button class="chip" @click="presetFirst30">{{ t('ai_first_30') }}</button>
                  </div>
                  <span class="ai-selected-count">{{ selectedCount }} {{ t('strings_word') }}</span>
              </div>

              <!-- ПРОМПТ (сворачиваемый) -->
              <div class="ai-disclosure">
                  <button class="ai-disclosure-head" @click="showPromptSettings = !showPromptSettings">
                      <Icon name="edit" :size="14" />
                      <span>{{ t('ai_prompt_settings') }}</span>
                      <span class="ai-chevron">{{ showPromptSettings ? '▾' : '▸' }}</span>
                  </button>
                  <div v-if="showPromptSettings" class="ai-disclosure-body">
                      <label class="ai-field-label">{{ t('ai_system_label') }}</label>
                      <textarea class="ai-system-input" v-model="ollamaSystem" @change="saveAiSettings" rows="8"></textarea>
                      <p class="ai-prompt-hint">{{ t('ai_prompt_hint') }}</p>
                      <div class="ai-chunk-row" v-if="aiTab !== 'manual'">
                          <label>{{ t('ai_chunk_size') }}</label>
                          <input type="number" v-model="chunkSize" min="1" max="200" @change="saveAiSettings" />
                          <span class="ai-prompt-hint" style="margin:0;">{{ t('ai_chunk_hint') }}</span>
                      </div>
                      <div class="ai-disclosure-foot">
                          <label class="toggle-hidden" style="margin: 0;">
                              <input type="checkbox" v-model="includeSpeaker" @change="saveAiSettings" />
                              {{ t('ai_include_speaker') }}
                          </label>
                          <button class="btn btn-secondary" @click="resetPrompt">{{ t('ai_reset_prompt') }}</button>
                      </div>
                  </div>
              </div>

              <!-- РЕЖИМ: OLLAMA -->
              <template v-if="aiTab === 'ollama'">
                  <div class="ai-settings-grid">
                      <label>URL</label>
                      <input type="text" v-model="ollamaUrl" />
                      <label>{{ t('ai_model') }}</label>
                      <input type="text" v-model="ollamaModel" />
                      <label>{{ t('ai_temperature') }}</label>
                      <div class="ai-temp-row">
                          <input type="range" v-model="ollamaTemp" min="0" max="1.5" step="0.05" @change="saveAiSettings" />
                          <span class="ai-temp-val">{{ Number(ollamaTemp).toFixed(2) }}</span>
                      </div>
                  </div>
                  <p class="ai-prompt-hint ai-ollama-hint">{{ t('ai_ollama_hint') }}</p>
              </template>

              <!-- РЕЖИМ: ОБЛАЧНЫЙ API -->
              <template v-if="aiTab === 'api'">
                  <div class="ai-settings-grid">
                      <label>URL</label>
                      <input type="text" v-model="apiUrl" @change="saveApiSettings" placeholder="https://api.openai.com/v1" />
                      <label>{{ t('ai_api_key') }}</label>
                      <input type="password" v-model="apiKey" @change="saveApiSettings" placeholder="sk-..." autocomplete="off" />
                      <label>{{ t('ai_model') }}</label>
                      <input type="text" v-model="apiModel" @change="saveApiSettings" placeholder="gpt-4o-mini" />
                      <label>{{ t('ai_temperature') }}</label>
                      <div class="ai-temp-row">
                          <input type="range" v-model="apiTemp" min="0" max="1.5" step="0.05" @change="saveApiSettings" />
                          <span class="ai-temp-val">{{ Number(apiTemp).toFixed(2) }}</span>
                      </div>
                  </div>
                  <p class="ai-prompt-hint ai-ollama-hint">{{ t('ai_api_hint') }}</p>
              </template>

              <!-- РЕЖИМ: ВРУЧНУЮ -->
              <template v-if="aiTab === 'manual'">
                  <div class="ai-step">
                      <span class="ai-step-badge">1</span>
                      <div class="ai-step-body">
                          <div class="ai-step-title">{{ t('step_1') }}</div>
                          <p>{{ t('step_1_desc') }}</p>
                          <button class="btn btn-secondary" @click="prepareAiBatch()"><Icon name="copy" :size="14" /> {{ t('copy_ai') }}</button>
                      </div>
                  </div>
                  <div class="ai-step">
                      <span class="ai-step-badge">2</span>
                      <div class="ai-step-body">
                          <div class="ai-step-title">{{ t('step_2') }}</div>
                          <p>{{ t('step_2_desc') }}</p>
                          <textarea v-model="aiInput" placeholder="1. Hello, world!&#10;2. How are you?"></textarea>
                      </div>
                  </div>
              </template>
          </div>

          <!-- ЗАКРЕПЛЁННЫЙ ФУТЕР -->
          <div class="ai-footer">
              <span class="ai-footer-info">{{ t('target_lang') }}: <b>{{ targetLang }}</b></span>
              <button v-if="(isOllamaTranslating || isApiTranslating) && !aiCancel" class="btn btn-secondary" @click="cancelAi">{{ t('cancel_btn') }}</button>
              <button v-if="aiTab === 'ollama'" class="btn btn-primary ai-run-btn" @click="runLocalLLM" :disabled="isOllamaTranslating || selectedCount === 0">
                  <span v-if="isOllamaTranslating" class="btn-spinner"></span>
                  {{ isOllamaTranslating ? t('ai_processing') : t('ai_translate_btn') }}
              </button>
              <button v-else-if="aiTab === 'api'" class="btn btn-primary ai-run-btn" @click="runApiLLM" :disabled="isApiTranslating || selectedCount === 0">
                  <span v-if="isApiTranslating" class="btn-spinner"></span>
                  {{ isApiTranslating ? t('ai_api_processing') : t('ai_api_translate_btn') }}
              </button>
              <button v-else class="btn btn-primary ai-run-btn" @click="importAiBatch" :disabled="!aiInput.trim()">
                  {{ t('apply_ai') }}
              </button>
          </div>
      </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { isAiModalOpen, parsedBlocks, targetLang, glossary, charMap, showMsg, currentFilePath, editorDirty, editorResizeTick } from '../store.js';
import { getBlockStatus } from '../actions.js';
import { stripLeadingPrefix } from '../diagnostics.js';
import { t } from '../locales.js';
import Icon from './Icon.vue';

const DEFAULT_SYSTEM = `You are a professional translator for visual novels. Translate each line into {target_lang}.

RULES:
- KEEP ALL TAGS EXACTLY AS-IS: [name], [player], {b}, {i}, {color=#fff}, {size=20}, \\n, etc.
- Do NOT translate variable names inside square brackets.
- Output ONLY a numbered list matching the input numbering. No introductions, no explanations, no quotes around lines.
- Output exactly {count} lines, numbered 1 to {count}.
- The [Speaker] prefix before each line is context only — NEVER include it in your translation.
- Preserve the original tone and meaning.

{glossary}`;

const aiTab = ref('ollama');
const aiStart = ref(1);
const aiEnd = ref(30);
const aiInput = ref('');
const showPromptSettings = ref(false);
const onlyUntranslated = ref(false);

const _batchStore = { ids: [] };

const ollamaUrl = ref(localStorage.getItem('renforge_ollama_url') || 'http://localhost:11434');
const ollamaModel = ref(localStorage.getItem('renforge_ollama_model') || 'llama3');
const ollamaTemp = ref(parseFloat(localStorage.getItem('renforge_ollama_temp')) || 0.3);
const ollamaSystem = ref(localStorage.getItem('renforge_ollama_system') || DEFAULT_SYSTEM);
const includeSpeaker = ref(localStorage.getItem('renforge_ai_include_speaker') !== 'false');
const isOllamaTranslating = ref(false);
// Размер чанка для автоматического перевода (Ollama/API): большие диапазоны бьём на
// порции, иначе упираемся в лимит контекста/таймаут и ломаем разбор нумерованного списка.
const chunkSize = ref(parseInt(localStorage.getItem('renforge_ai_chunk')) || 25);

// Облачный OpenAI-совместимый API
const apiUrl = ref(localStorage.getItem('renforge_api_url') || 'https://api.openai.com/v1');
const apiKey = ref(localStorage.getItem('renforge_api_key') || '');
const apiModel = ref(localStorage.getItem('renforge_api_model') || 'gpt-4o-mini');
const apiTemp = ref(parseFloat(localStorage.getItem('renforge_api_temp')) || 0.3);
const isApiTranslating = ref(false);
// Флаг отмены чанкового перевода: проверяется между порциями (текущая порция дорабатывается).
const aiCancel = ref(false);
function cancelAi() { aiCancel.value = true; }

onMounted(() => {
    aiEnd.value = Math.min(30, parsedBlocks.value.length);
});

// --- Пресеты диапазона ---
function presetAll() { aiStart.value = 1; aiEnd.value = parsedBlocks.value.length; onlyUntranslated.value = false; }
function presetFirst30() { aiStart.value = 1; aiEnd.value = Math.min(30, parsedBlocks.value.length); onlyUntranslated.value = false; }
function presetUntranslated() { aiStart.value = 1; aiEnd.value = parsedBlocks.value.length; onlyUntranslated.value = true; }

const selectedCount = computed(() => {
    let s = parseInt(aiStart.value) - 1, e = parseInt(aiEnd.value);
    const len = parsedBlocks.value.length;
    if (s < 0) s = 0; if (e > len) e = len; if (s >= e) return 0;
    let b = parsedBlocks.value.slice(s, e);
    if (onlyUntranslated.value) b = b.filter(x => getBlockStatus(x) !== 'translated');
    return b.length;
});

function saveAiSettings() {
    localStorage.setItem('renforge_ollama_system', ollamaSystem.value);
    localStorage.setItem('renforge_ollama_temp', ollamaTemp.value);
    localStorage.setItem('renforge_ai_include_speaker', includeSpeaker.value ? 'true' : 'false');
    localStorage.setItem('renforge_ai_chunk', chunkSize.value);
}
function resetPrompt() { ollamaSystem.value = DEFAULT_SYSTEM; saveAiSettings(); }

function updateStats() {
    // Перевод применён в память, но ещё не сохранён в БД — помечаем как «не сохранено».
    // fileStats (прогресс на дашборде) двигается только при сохранении/обновлении из БД.
    editorDirty.value = true;
}

function speakerOf(block) {
    let charInfo = block.who || 'Narrator';
    if (charInfo && charMap.value[charInfo]) charInfo = charMap.value[charInfo];
    if (!charInfo || charInfo === 'None') charInfo = 'Narrator';
    return charInfo;
}

function buildBatch() {
    let s = parseInt(aiStart.value) - 1, e = parseInt(aiEnd.value);
    const len = parsedBlocks.value.length;
    if (s < 0) s = 0; if (e > len) e = len; if (s >= e) return null;
    let batch = parsedBlocks.value.slice(s, e);
    if (onlyUntranslated.value) batch = batch.filter(x => getBlockStatus(x) !== 'translated');
    if (!batch.length) return null;
    _batchStore.ids = batch.map(b => b.id);

    let linesText = '';
    batch.forEach((b, i) => {
        if (includeSpeaker.value) linesText += `${i + 1}. [${speakerOf(b)}]: ${b.original}\n`;
        else linesText += `${i + 1}. ${b.original}\n`;
    });
    return { batch, linesText };
}

// Блоки текущего выбора (диапазон + фильтр «только непереведённые») — без сборки текста.
function getSelectedBlocks() {
    let s = parseInt(aiStart.value) - 1, e = parseInt(aiEnd.value);
    const len = parsedBlocks.value.length;
    if (s < 0) s = 0; if (e > len) e = len; if (s >= e) return [];
    let batch = parsedBlocks.value.slice(s, e);
    if (onlyUntranslated.value) batch = batch.filter(x => getBlockStatus(x) !== 'translated');
    return batch;
}

// Текст нумерованного списка для произвольного набора блоков (нумерация с 1 внутри чанка).
function buildLinesFor(chunk) {
    let linesText = '';
    chunk.forEach((b, i) => {
        if (includeSpeaker.value) linesText += `${i + 1}. [${speakerOf(b)}]: ${b.original}\n`;
        else linesText += `${i + 1}. ${b.original}\n`;
    });
    return linesText;
}

// Автоматический перевод порциями. sendFn(system, userPrompt) -> текст ответа модели.
// Большие диапазоны бьём на чанки (chunkSize), чтобы не упираться в контекст/таймаут и
// чтобы разбор нумерованного списка не разваливался. Частичный результат сохраняется.
async function runChunked(sendFn, busyRef) {
    const blocks = getSelectedBlocks();
    if (!blocks.length) { showMsg('error', 'Invalid interval!'); return; }
    const size = Math.max(1, parseInt(chunkSize.value) || 25);
    const chunks = [];
    for (let i = 0; i < blocks.length; i += size) chunks.push(blocks.slice(i, i + size));

    busyRef.value = true;
    aiCancel.value = false;
    let total = 0;
    let missing = 0;
    let cancelled = false;
    try {
        for (let ci = 0; ci < chunks.length; ci++) {
            if (aiCancel.value) { cancelled = true; break; }
            const chunk = chunks[ci];
            _batchStore.ids = chunk.map(b => b.id);
            const system = buildSystem(chunk.length);
            const userPrompt = `Translate the following numbered lines. Output ONLY the numbered translations.\n\nLINES TO TRANSLATE:\n${buildLinesFor(chunk)}`;
            if (chunks.length > 1) showMsg('success', `${t('ai_chunk')} ${ci + 1}/${chunks.length}...`, 0);
            const resp = await sendFn(system, userPrompt);
            const applied = parseAiResponseAndApply(resp);
            total += applied;
            // Контроль выравнивания: модель должна вернуть РОВНО столько строк, сколько в чанке.
            // Недобор = пропуск/перенумерация ответа -> часть строк могла лечь не туда.
            if (applied < chunk.length) missing += (chunk.length - applied);
        }
        editorResizeTick.value++;
        updateStats();
        if (cancelled) {
            showMsg('success', `${t('ai_cancelled')} ${t('msg_ai_applied')} ${total}`, 8000);
        } else if (missing > 0) {
            // Не тихо: предупреждаем, что часть строк не переведена/возможен сдвиг — проверить.
            showMsg('error', `${t('msg_ai_applied')} ${total}. ${t('ai_mismatch_warn').replace('{n}', missing)}`, 15000);
        } else {
            showMsg('success', `${t('msg_ai_applied')} ${total}`);
        }
        if (!cancelled) isAiModalOpen.value = false;
    } catch (e) {
        editorResizeTick.value++;
        if (total > 0) updateStats();
        const tail = total > 0 ? ` (${t('ai_applied_before_fail')} ${total})` : '';
        showMsg('error', (e?.message || e) + tail, 15000);
    } finally {
        busyRef.value = false;
        aiCancel.value = false;
    }
}

function buildSystem(count) {
    let g = '';
    if (glossary.value.length > 0) {
        g = 'GLOSSARY (use these exact terms):\n';
        glossary.value.forEach(term => { g += `  ${term.original} → ${term.translation}\n`; });
    }
    return (ollamaSystem.value || DEFAULT_SYSTEM)
        .split('{target_lang}').join(targetLang.value)
        .split('{count}').join(String(count))
        .split('{glossary}').join(g);
}

// Ручной режим: копирует системный промпт + нумерованный список выбранных строк в буфер.
async function prepareAiBatch() {
    const built = buildBatch();
    if (!built) { showMsg('error', 'Invalid interval!'); return; }
    const system = buildSystem(built.batch.length);
    const combined = `${system}\n\nLINES TO TRANSLATE:\n${built.linesText}`;
    try {
        await navigator.clipboard.writeText(combined);
        showMsg('success', t('msg_copy_success'));
    } catch (e) { showMsg('error', t('msg_copy_err')); }
}

function parseAiResponseAndApply(text) {
    const batchIds = _batchStore.ids;
    if (!batchIds.length) return 0;

    const entries = [];
    const numPattern = /^\s*#?(\d+)[\.\)\:]\s*/;
    const lines = text.split('\n');

    let currentNum = -1, currentText = '', foundStart = false;
    for (const line of lines) {
        const match = line.match(numPattern);
        if (match) {
            const num = parseInt(match[1]);
            if (!foundStart && num === 1) {
                foundStart = true; currentNum = 1;
                currentText = line.replace(numPattern, '').trim();
            } else if (foundStart && num === currentNum + 1) {
                if (currentText) entries.push(currentText.replace(/^["'`]|["'`]$/g, '').trim());
                currentNum = num;
                currentText = line.replace(numPattern, '').trim();
            } else if (foundStart) {
                currentText += '\n' + line.trim();
            }
        } else if (foundStart && line.trim()) {
            currentText += '\n' + line.trim();
        }
    }
    if (currentText) entries.push(currentText.replace(/^["'`]|["'`]$/g, '').trim());

    let appliedCount = 0;
    batchIds.forEach((id, index) => {
        if (entries[index]) {
            const block = parsedBlocks.value.find(b => b.id === id);
            if (block) {
                // Авто-зачистка прилипшего ведущего префикса-эхо ([ENGINE]: / [name]:) —
                // универсально, во всех пачках; срезаем только если такого бракета нет в
                // начале оригинала (легитимный ведущий [var] не трогаем).
                const raw = entries[index];
                const stripped = stripLeadingPrefix(raw, block.original);
                block.translation = stripped != null ? stripped : raw;
                appliedCount++;
            }
        }
    });
    return appliedCount;
}

function importAiBatch() {
    const applied = parseAiResponseAndApply(aiInput.value);
    editorResizeTick.value++;
    showMsg('success', `${t('msg_ai_applied')} ${applied}`);
    updateStats();
    isAiModalOpen.value = false;
}

function saveApiSettings() {
    localStorage.setItem('renforge_api_url', apiUrl.value);
    localStorage.setItem('renforge_api_key', apiKey.value);
    localStorage.setItem('renforge_api_model', apiModel.value);
    localStorage.setItem('renforge_api_temp', apiTemp.value);
}

async function runApiLLM() {
    saveApiSettings();
    await runChunked(async (system, userPrompt) => {
        return await invoke('llm_chat_request', {
            baseUrl: apiUrl.value,
            apiKey: apiKey.value,
            model: apiModel.value,
            system,
            user: userPrompt,
            temperature: parseFloat(apiTemp.value) || 0,
        });
    }, isApiTranslating);
}

async function runLocalLLM() {
    localStorage.setItem('renforge_ollama_url', ollamaUrl.value);
    localStorage.setItem('renforge_ollama_model', ollamaModel.value);
    saveAiSettings();
    await runChunked(async (system, userPrompt) => {
        const res = await fetch(`${ollamaUrl.value}/api/generate`, {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model: ollamaModel.value,
                system,
                prompt: userPrompt,
                stream: false,
                options: { num_predict: -1, num_ctx: 8192, temperature: parseFloat(ollamaTemp.value) || 0 }
            })
        });
        if (!res.ok) throw new Error(`Ollama HTTP ${res.status}`);
        const data = await res.json();
        return data.response;
    }, isOllamaTranslating);
}

</script>
