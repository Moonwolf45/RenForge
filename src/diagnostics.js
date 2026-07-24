// Единый реестр диагностик строки перевода + автофиксы (по образцу линтера/code actions).
// Цель: одно место для всех проверок (ошибки/предупреждения) и их починок. Добавить новую
// проверку = дописать объект-правило. Результат diagnose() мемоизируется по содержимому
// блока, чтобы не пересчитывать на каждый ререндер (как кэш подсветки глоссария).

// --- Извлечение токенов ---
export function extractTags(text) { return (text || '').match(/(\[.*?\]|\{.*?\})/g) || []; }
export function extractInterps(text) { return (text || '').match(/\[.*?\]/g) || []; }
export function getOriginalTags(block) { return extractTags(block.original || ''); }

export function getMissingTags(block) {
  if (!block.original || !block.translation) return [];
  return extractTags(block.original).filter(tag => !block.translation.includes(tag));
}

// Лишние интерполяции [var] в переводе, которых НЕТ в оригинале. Почти всегда ошибка
// (несуществующая переменная -> Ren'Py KeyError). {текст-теги} не проверяем (их можно добавлять).
export function getExtraInterps(block) {
  if (!block.original || !block.translation) return [];
  const orig = extractInterps(block.original);
  return extractInterps(block.translation).filter(tag => !orig.includes(tag));
}

// --- Фикс 1: срезать «прилипший» ведущий префикс-эхо вида [ENGINE]: / [name] ---
// Срабатывает ТОЛЬКО если такого ведущего бракета нет в начале оригинала — чтобы не
// тронуть легитимный ведущий [var] (напр. оригинал и перевод оба начинаются с [player]).
const LEADING_PREFIX_RE = /^\s*\[[^\]]*\]\s*:?\s*/;
function leadingBracket(text) { const m = (text || '').match(/^\s*(\[[^\]]*\])/); return m ? m[1] : null; }
export function stripLeadingPrefix(translation, original) {
  const tr = translation || '';
  const origLead = leadingBracket(original || '');
  let cur = tr;
  let changed = false;
  // Срезаем ВСЕ ведущие префиксы-эхо подряд ([A]:[B]: текст). Останавливаемся, если
  // текущий ведущий бракет совпадает с ведущим бракетом оригинала (легитимный [var]).
  while (true) {
    const lead = leadingBracket(cur);
    if (!lead) break;
    if (origLead && origLead === lead) break;
    if (!LEADING_PREFIX_RE.test(cur)) break;
    const next = cur.replace(LEADING_PREFIX_RE, '');
    if (next === cur) break;
    cur = next;
    changed = true;
  }
  return changed ? cur : null;
}

// --- Фикс 2: восстановить потерянные ВЕДУЩИЕ токены оригинала ({#weekday}, {b}, [var]) ---
// Берём непрерывный ведущий ран токенов оригинала и, если ВСЕ они отсутствуют в переводе,
// подставляем их в начало (сохраняя исходный отступ-разделитель из оригинала).
const LEADING_TOKENS_RE = /^(?:\s*(?:\{[^}]*\}|\[[^\]]*\]))+\s*/;
function leadingTokenRun(text) { const m = (text || '').match(LEADING_TOKENS_RE); return m ? m[0] : ''; }
export function restoreLeadingToken(translation, original) {
  const lead = leadingTokenRun(original);
  const toks = lead.match(/\{[^}]*\}|\[[^\]]*\]/g) || [];
  if (!toks.length) return null;
  const tr = translation || '';
  if (!tr.trim()) return null;                       // пустой перевод не трогаем
  if (!toks.every(tok => !tr.includes(tok))) return null; // часть токенов уже на месте -> не дублируем
  return lead + tr.replace(/^\s+/, '');
}

// --- Предупреждение о длинном UI-переводе + автоперенос (перенесено из редактора) ---
const _measureCtx = (() => {
  try { return document.createElement('canvas').getContext('2d'); } catch (e) { return null; }
})();
function visibleWidth(line) {
  const visible = (line || '').replace(/\{[^}]*\}/g, '');
  if (!_measureCtx) return visible.length;
  _measureCtx.font = '20px sans-serif';
  return _measureCtx.measureText(visible).width;
}
export function uiOverflowWarn(block) {
  if (block.block_type !== 'ui') return false;
  const o = (block.original || '').trim();
  const tr = (block.translation || '').trim();
  if (!tr || tr === o) return false;
  if (tr.includes('\n')) return false;
  if (o.length < 6) return false;
  return tr.length > o.length * 1.3 && (tr.length - o.length) >= 4;
}
export function wrapToFit(block) {
  const origLines = (block.original || '').replace(/\\n/g, '\n').split('\n');
  let budget = 0;
  for (const l of origLines) budget = Math.max(budget, visibleWidth(l));
  if (budget <= 0) return null;
  const tr = (block.translation || '').replace(/\\n/g, '\n').replace(/\n/g, ' ').trim();
  if (!tr) return null;
  const words = tr.split(/\s+/);
  const lines = [];
  let cur = '';
  for (const wd of words) {
    const cand = cur ? cur + ' ' + wd : wd;
    if (cur && visibleWidth(cand) > budget) { lines.push(cur); cur = wd; }
    else cur = cand;
  }
  if (cur) lines.push(cur);
  const out = lines.join('\n');
  return out !== (block.translation || '') ? out : null;
}

// --- Реестр правил ---
// severity: 'error' блокирует сохранение (учитывается в blockStatus); 'warning' — мягко.
// test(block) -> массив «деталей» (пусто = проблемы нет).
// fix(block) -> исправленный перевод или null. Фикс контекстный: напр. у «лишних
// переменных» он срабатывает, только если это прилипший ВЕДУЩИЙ префикс-эхо ([ENGINE]:),
// а у «потерянных тегов» — только если потерян именно ВЕДУЩИЙ токен. Иначе вернёт null
// (чинить вручную) — и кнопка «Исправить» у такой строки не показывается.
// bulk: участвует ли в массовом «Исправить файл».
export const RULES = [
  { id: 'missing-tag',  severity: 'error', msgKey: 'tag_error',
    test: getMissingTags,
    fix: (b) => restoreLeadingToken(b.translation, b.original), bulk: true },
  { id: 'extra-interp', severity: 'error', msgKey: 'tag_error_extra',
    test: getExtraInterps,
    fix: (b) => stripLeadingPrefix(b.translation, b.original), bulk: true },
  { id: 'ui-overflow',  severity: 'warning', msgKey: 'ui_length_warn',
    test: (b) => uiOverflowWarn(b) ? ['ui'] : [],
    fix: (b) => wrapToFit(b), bulk: false },
];

// --- Мемоизация diagnose по содержимому блока ---
const _cache = new Map();
export function clearDiagnostics() { _cache.clear(); }

export function diagnose(block) {
  const key = block.id + '\u0000' + (block.original || '') + '\u0000' + (block.translation || '');
  let v = _cache.get(key);
  if (v !== undefined) return v;
  // Предохранитель от безграничного роста кэша в долгой сессии правки (новый ключ на
  // каждое изменение перевода). Сбрасываем при превышении порога — память важнее, чем
  // сохранение мемо для давно неактуальных состояний.
  if (_cache.size > 4000) _cache.clear();
  v = [];
  for (const r of RULES) {
    const items = r.test(block) || [];
    if (!items.length) continue;
    // fixable = фикс РЕАЛЬНО применим к этой строке (вернул непустой результат), а не
    // просто «у правила есть fix». Так кнопка не висит впустую на неисправимых случаях.
    const canFix = r.fix ? (r.fix(block) != null) : false;
    v.push({ id: r.id, severity: r.severity, msgKey: r.msgKey, items, fixable: canFix, bulkFix: canFix && r.bulk });
  }
  _cache.set(key, v);
  return v;
}

// Есть ли в файле строки, поддающиеся массовому автофиксу (для показа кнопки «Исправить файл»).
export function hasBulkFixables(blocks) {
  return (blocks || []).some(b => diagnose(b).some(d => d.bulkFix));
}

// Статус блока: error при любой диагностике severity=error; иначе по содержимому.
// confirmed (ручная отметка) бьёт правило «перевод == оригиналу», но НЕ бьёт ошибки.
export function blockStatus(block) {
  for (const d of diagnose(block)) if (d.severity === 'error') return 'error';
  const hasTr = !!(block.translation && block.translation.trim());
  if (block.confirmed && hasTr) return 'translated';
  if (!hasTr || block.translation === block.original) return 'untranslated';
  if (block.prev_original) return 'outdated';
  return 'translated';
}

// Применить фикс правила к блоку -> исправленный перевод или null.
export function applyFix(block, ruleId) {
  const r = RULES.find(x => x.id === ruleId);
  if (!r || !r.fix) return null;
  return r.fix(block);
}

// Массовая починка файла: применяет все bulk-фиксы ко всем блокам (мутирует translation).
// Возвращает число изменённых строк.
export function fixFile(blocks) {
  let n = 0;
  const bulkRules = RULES.filter(r => r.bulk && r.fix);
  for (const b of blocks) {
    let changed = false;
    for (const r of bulkRules) {
      const res = r.fix(b);
      if (res != null && res !== b.translation) { b.translation = res; changed = true; }
    }
    if (changed) n++;
  }
  return n;
}
