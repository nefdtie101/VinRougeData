// VinRouge DSL Editor — CodeMirror 5 bridge with IntelliSense
//
// Public API:
//   vrDslInit(id, code)      — mount editor in element #id
//   vrDslSetValue(id, code)  — programmatic content update
//   vrDslGetValue(id)        — read current content
//   vrDslDestroy(id)         — unmount
//   vrDslSetSchema(json)     — load [{table_name, columns:[]}] for IntelliSense

const _vrEditors = {};

// ── Schema store (fed from Rust via vrDslSetSchema) ───────────────────────────
// shape: [{ table_name: string, columns: string[] }]
let _schema = [];

// ── DSL keyword sets ──────────────────────────────────────────────────────────

const KEYWORDS = new Set([
  'ASSERT','SAMPLE','SUM','AVG','COUNT','MIN','MAX',
  'WHERE','FROM','SIZE','TOP','DISTINCT',
  'MUS','RANDOM','SYSTEMATIC','STRATIFIED',
  'RELATION','CHART','SCREEN','BY','SCHEMA',
]);

const BUILTINS = new Set([
  'UPPER','LOWER','TRIM','LENGTH','DATE',
  'CASE','WHEN','THEN','ELSE','END',
  'COALESCE','NULLIF','IIF','ABS','ROUND',
  'COUNTIF','SUMIF',
  'IS_BLANK','IS_NUMERIC','IS_DATE','DUPLICATED',
]);

const ATOMS = new Set([
  'AND','OR','NOT','IN','BETWEEN','IS','LIKE','NULL','TRUE','FALSE',
]);

// Signature hints shown below the cursor when typing a function call
const FN_SIGNATURES = {
  ASSERT:     { sig: 'ASSERT <expr>',                           doc: 'Asserts that the expression holds for all rows.' },
  SUM:        { sig: 'SUM(table.column)',                       doc: 'Sum of all values in a column.' },
  AVG:        { sig: 'AVG(table.column)',                       doc: 'Average of all values in a column.' },
  COUNT:      { sig: 'COUNT(table.column)',                     doc: 'Count of non-null values.' },
  MIN:        { sig: 'MIN(table.column)',                       doc: 'Minimum value in a column.' },
  MAX:        { sig: 'MAX(table.column)',                       doc: 'Maximum value in a column.' },
  SAMPLE:     { sig: 'SAMPLE MUS|RANDOM|SYSTEMATIC SIZE n FROM table WHERE <cond>', doc: 'Draw an audit sample from a table.' },
  UPPER:      { sig: 'UPPER(table.column)',                     doc: 'Convert string to upper-case.' },
  LOWER:      { sig: 'LOWER(table.column)',                     doc: 'Convert string to lower-case.' },
  TRIM:       { sig: 'TRIM(table.column)',                      doc: 'Strip leading/trailing whitespace.' },
  LENGTH:     { sig: 'LENGTH(table.column)',                    doc: 'String length in characters.' },
  DATE:       { sig: 'DATE(table.column)',                      doc: 'Normalise value to a date.' },
  COALESCE:   { sig: 'COALESCE(a, b, ...)',                    doc: 'First non-null argument.' },
  NULLIF:     { sig: 'NULLIF(a, b)',                            doc: 'Returns null when a = b, else a.' },
  IIF:        { sig: 'IIF(condition, then, else)',              doc: 'Inline conditional.' },
  ABS:        { sig: 'ABS(table.column)',                       doc: 'Absolute value.' },
  ROUND:      { sig: 'ROUND(table.column, decimals)',           doc: 'Round to N decimal places.' },
  COUNTIF:    { sig: 'COUNTIF(table.column, criteria)',         doc: 'Count rows matching a criteria.' },
  SUMIF:      { sig: 'SUMIF(range.column, criteria, sum.column)', doc: 'Sum rows matching a criteria.' },
  CASE:       { sig: 'CASE WHEN <cond> THEN <val> … ELSE <val> END', doc: 'Conditional expression.' },
  RELATION:   { sig: 'RELATION table1.col -> table2.col',        doc: 'Declare a foreign-key mapping between two columns.' },
  SCHEMA:     { sig: 'SCHEMA',                                    doc: 'Print the name, columns and row count of every imported table.' },
  CHART:      { sig: 'CHART <type> <aggregate> BY <dimension>',  doc: 'Define a chart output with grouped aggregate data.' },
  SCREEN:     { sig: 'SCREEN "title" { CHART ... }',             doc: 'Group multiple charts into a dashboard screen.' },
  IS_BLANK:   { sig: 'IS_BLANK(table.column)',                   doc: 'True when value is null or empty string.' },
  IS_NUMERIC: { sig: 'IS_NUMERIC(table.column)',                 doc: 'True when value can be parsed as a number.' },
  IS_DATE:    { sig: 'IS_DATE(table.column)',                    doc: 'True when value matches a recognised date format.' },
  DUPLICATED: { sig: 'DUPLICATED(table.col1, table.col2, ...)', doc: 'True when the composite key occurs more than once in the table.' },
};

const KW_COMPLETIONS = [
  ...Array.from(KEYWORDS), ...Array.from(BUILTINS), ...Array.from(ATOMS),
].sort();

// ── Custom DSL tokeniser mode ─────────────────────────────────────────────────

function defineVrDslMode() {
  if (CodeMirror.modes['vrdsl']) return;   // already defined

  CodeMirror.defineMode('vrdsl', function() {
    return {
      startState: () => ({ inString: null }),

      token(stream, state) {
        if (state.inString) {
          while (!stream.eol()) {
            if (stream.next() === state.inString) { state.inString = null; break; }
          }
          return 'string';
        }

        if (stream.eatSpace()) return null;

        // Line comment
        if (stream.match('--')) { stream.skipToEnd(); return 'comment'; }

        // String literal
        const q = stream.peek();
        if (q === '"' || q === "'") {
          stream.next();
          state.inString = q;
          while (!stream.eol()) {
            if (stream.next() === q) { state.inString = null; break; }
          }
          return 'string';
        }

        // Number
        if (stream.match(/^-?\d+(\.\d+)?/)) return 'number';

        // Word (keyword / function / identifier / table.column)
        if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*/)) {
          const word = stream.current().toUpperCase();
          if (KEYWORDS.has(word)) return 'keyword';
          if (BUILTINS.has(word)) return 'builtin';
          if (ATOMS.has(word))    return 'atom';
          // label: declaration
          if (stream.peek() === ':') return 'def';
          // table.column — consume dot + column
          if (stream.peek() === '.') {
            stream.next();
            stream.match(/^[A-Za-z0-9_]*/);
            return 'variable-2';
          }
          return 'variable';
        }

        // Operators
        if (stream.match(/^(<>|>=|<=|<|>|=|\+|-|\*|\/|%)/)) return 'operator';

        stream.next();
        return null;
      },
    };
  });
}

// ── Theme CSS ─────────────────────────────────────────────────────────────────

const THEME_CSS = `
.cm-s-vinrouge.CodeMirror {
  background: #0d0a0b;
  color: #f0e6e8;
  font-family: ui-monospace, "Cascadia Code", "Fira Code", monospace;
  font-size: 13px;
  line-height: 1.75;
  height: 100%;
  border: none;
}
.cm-s-vinrouge .CodeMirror-scroll { padding-bottom: 24px; }
.cm-s-vinrouge .CodeMirror-gutters {
  background: #100c0d;
  border-right: 1px solid #2a1419;
  color: #4a3238;
  min-width: 40px;
}
.cm-s-vinrouge .CodeMirror-linenumber { padding: 0 10px 0 6px; }
.cm-s-vinrouge .CodeMirror-cursor { border-left: 2px solid #c84b5e; }
.cm-s-vinrouge .CodeMirror-selected { background: #3a1f25; }
.cm-s-vinrouge.CodeMirror-focused .CodeMirror-selected { background: #3a1f25; }
.cm-s-vinrouge .CodeMirror-activeline-background { background: #160f11; }
.cm-s-vinrouge .CodeMirror-matchingbracket { color: #fbbf24 !important; text-decoration: underline; }

.cm-s-vinrouge .cm-keyword   { color: #c084fc; font-weight: 600; }
.cm-s-vinrouge .cm-builtin   { color: #60a5fa; }
.cm-s-vinrouge .cm-atom      { color: #f472b6; }
.cm-s-vinrouge .cm-string    { color: #86efac; }
.cm-s-vinrouge .cm-number    { color: #fbbf24; }
.cm-s-vinrouge .cm-comment   { color: #6b4b52; font-style: italic; }
.cm-s-vinrouge .cm-operator  { color: #f9a8d4; }
.cm-s-vinrouge .cm-variable  { color: #e2d0d4; }
.cm-s-vinrouge .cm-variable-2{ color: #c9a8ae; }
.cm-s-vinrouge .cm-def       { color: #a78bfa; font-style: italic; }

/* ── Hint dropdown ── */
.CodeMirror-hints {
  background: #1a1014 !important;
  border: 1px solid #3a1f25 !important;
  border-radius: 8px !important;
  box-shadow: 0 12px 36px rgba(0,0,0,0.65) !important;
  font-family: ui-monospace, "Cascadia Code", "Fira Code", monospace;
  font-size: 12px;
  z-index: 9999;
  padding: 4px 0 !important;
  min-width: 300px;
  max-height: 280px !important;
  overflow-y: auto !important;
}
.CodeMirror-hint {
  color: #c9a8ae !important;
  padding: 0 !important;
  line-height: 1.4;
}
li.CodeMirror-hint-active {
  background: #2d1620 !important;
  color: #f0e6e8 !important;
  outline: none !important;
}
li.CodeMirror-hint-active .vr-hint-name { color: #f0e6e8; }

.vr-hint-icon { display: inline-flex; align-items: center; justify-content: center; }
.vr-hint-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #c9a8ae; }
.vr-hint-meta { font-size: 10px; color: #4a3040; margin-left: auto; flex-shrink: 0; white-space: nowrap; }

/* ── Signature tooltip ── */
.vr-sig-tooltip {
  position: absolute;
  z-index: 9998;
  background: #1e1316;
  border: 1px solid #522530;
  border-radius: 6px;
  padding: 8px 12px;
  font-family: ui-monospace, monospace;
  font-size: 12px;
  color: #c9a8ae;
  pointer-events: none;
  max-width: 420px;
  box-shadow: 0 6px 20px rgba(0,0,0,0.5);
  line-height: 1.5;
}
.vr-sig-tooltip .vr-sig-sig   { color: #f0e6e8; font-weight: 500; margin-bottom: 3px; }
.vr-sig-tooltip .vr-sig-doc   { color: #9a7e84; font-size: 11px; font-family: system-ui, sans-serif; }
`;

function injectTheme() {
  if (document.getElementById('vr-dsl-theme')) return;
  const s = document.createElement('style');
  s.id = 'vr-dsl-theme';
  s.textContent = THEME_CSS;
  document.head.appendChild(s);
}

// ── Context-aware IntelliSense hint function ──────────────────────────────────

function dslHint(cm) {
  const cur  = cm.getCursor();
  const line = cm.getLine(cur.line);

  // ── Case 1: table.| — column completion (like SQL editor) ────────────────
  const dotMatch = line.slice(0, cur.ch).match(/([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z0-9_]*)$/);
  if (dotMatch) {
    const tableName = dotMatch[1];
    const colPrefix = dotMatch[2].toLowerCase();
    const colStart  = cur.ch - dotMatch[2].length;

    const tableSchema = _schema.find(s =>
      s.table_name.toLowerCase() === tableName.toLowerCase()
    );

    // No matching table — still return an empty result so CodeMirror knows
    // we handled this position (prevents keyword list from bleeding in).
    const cols = tableSchema
      ? tableSchema.columns
          .filter(c => !colPrefix || c.toLowerCase().startsWith(colPrefix))
          .map(c => ({
            text: c,
            displayText: c,
            _table: tableName,
            className: 'vr-hint-col',
          }))
      : [];

    return {
      list: cols,
      from: CodeMirror.Pos(cur.line, colStart),
      to:   CodeMirror.Pos(cur.line, cur.ch),
    };
  }

  // ── Case 2: word completion ───────────────────────────────────────────────
  let start = cur.ch;
  while (start > 0 && /[A-Za-z_0-9]/.test(line[start - 1])) start--;
  const word   = line.slice(start, cur.ch);
  const wordUp = word.toUpperCase();
  const wordLo = word.toLowerCase();

  // Tables always appear — exactly like a SQL editor.
  // They appear at the top of the list, filtered by the current prefix.
  const tables = _schema
    .filter(s => !wordLo || s.table_name.toLowerCase().startsWith(wordLo))
    .map(s => ({
      text: s.table_name,
      displayText: s.table_name,
      _colCount: s.columns.length,
      className: 'vr-hint-table',
    }));

  // Keywords / builtins / atoms — filtered by prefix, deduped against tables.
  const tableNames = new Set(tables.map(t => t.text.toUpperCase()));
  const kws = KW_COMPLETIONS
    .filter(k => (!wordUp || k.startsWith(wordUp)) && !tableNames.has(k))
    .map(k => {
      const type = KEYWORDS.has(k) ? 'vr-hint-kw'
                 : BUILTINS.has(k) ? 'vr-hint-fn'
                 : 'vr-hint-atom';
      return { text: k, displayText: k, className: type };
    });

  return {
    list: [...tables, ...kws],
    from: CodeMirror.Pos(cur.line, start),
    to:   CodeMirror.Pos(cur.line, cur.ch),
  };
}

// ── Signature tooltip ─────────────────────────────────────────────────────────

let _sigTooltip = null;

function hideSig() {
  if (_sigTooltip) { _sigTooltip.remove(); _sigTooltip = null; }
}

function showSig(cm, fnName) {
  hideSig();
  const info = FN_SIGNATURES[fnName.toUpperCase()];
  if (!info) return;

  const coords = cm.cursorCoords(true, 'page');
  const tip = document.createElement('div');
  tip.className = 'vr-sig-tooltip';
  tip.innerHTML =
    `<div class="vr-sig-sig">${info.sig}</div>` +
    `<div class="vr-sig-doc">${info.doc}</div>`;
  tip.style.left = `${coords.left}px`;
  tip.style.top  = `${coords.bottom + 4}px`;
  document.body.appendChild(tip);
  _sigTooltip = tip;
}

function checkSignature(cm) {
  const cur  = cm.getCursor();
  const line = cm.getLine(cur.line);
  const up   = line.slice(0, cur.ch);

  // Find innermost open paren that has a known function before it
  let depth = 0;
  for (let i = up.length - 1; i >= 0; i--) {
    if (up[i] === ')') { depth++; continue; }
    if (up[i] === '(') {
      if (depth > 0) { depth--; continue; }
      // Look back for the function name
      const before = up.slice(0, i).trimEnd();
      const m = before.match(/([A-Za-z_][A-Za-z0-9_]*)$/);
      if (m && FN_SIGNATURES[m[1].toUpperCase()]) {
        showSig(cm, m[1]);
        return;
      }
      break;
    }
  }

  // Also show for ASSERT / SAMPLE at line start
  const kwMatch = up.trimStart().match(/^(ASSERT|SAMPLE)\b/i);
  if (kwMatch) { showSig(cm, kwMatch[1]); return; }

  hideSig();
}

// ── Hint item renderer — SQL-editor style ────────────────────────────────────

const HINT_ICONS = {
  'vr-hint-table': {
    color: '#60a5fa',
    svg: `<svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <rect x="1" y="1" width="11" height="11" rx="1.5" stroke="currentColor" stroke-width="1.1"/>
      <path d="M1 4.5h11M4.5 4.5v7" stroke="currentColor" stroke-width="1.1"/>
    </svg>`,
  },
  'vr-hint-col': {
    color: '#86efac',
    svg: `<svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <path d="M2.5 3.5h8M2.5 6.5h5.5M2.5 9.5h7"
        stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
    </svg>`,
  },
  'vr-hint-fn': {
    color: '#60a5fa',
    svg: `<svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <path d="M3 10c.5-3 1-5 2-7M5 5.5h4M6 8.5h3"
        stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
    </svg>`,
  },
  'vr-hint-kw': {
    color: '#c084fc',
    svg: `<svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <rect x="1.5" y="3.5" width="10" height="6" rx="1.5"
        stroke="currentColor" stroke-width="1.1"/>
      <path d="M4 6.5h5M6.5 5v3"
        stroke="currentColor" stroke-width="1" stroke-linecap="round"/>
    </svg>`,
  },
  'vr-hint-atom': {
    color: '#f472b6',
    svg: `<svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <circle cx="6.5" cy="6.5" r="2" stroke="currentColor" stroke-width="1.1"/>
      <circle cx="6.5" cy="6.5" r="5" stroke="currentColor" stroke-width="1.1"
        stroke-dasharray="3 2"/>
    </svg>`,
  },
};

function renderHintItem(elt, data, cur) {
  elt.style.display = 'flex';
  elt.style.alignItems = 'center';
  elt.style.gap = '7px';
  elt.style.padding = '4px 12px';

  // Icon
  const ico = document.createElement('span');
  ico.className = 'vr-hint-icon';
  const def = HINT_ICONS[cur.className] || HINT_ICONS['vr-hint-kw'];
  ico.innerHTML = def.svg;
  ico.style.color = def.color;
  ico.style.flexShrink = '0';
  ico.style.lineHeight = '0';
  elt.appendChild(ico);

  // Name
  const name = document.createElement('span');
  name.className = 'vr-hint-name';
  name.textContent = cur.displayText || cur.text;
  elt.appendChild(name);

  // Right-side meta: column count for tables, table name for columns
  if (cur.className === 'vr-hint-table' && cur._colCount != null) {
    const badge = document.createElement('span');
    badge.className = 'vr-hint-meta';
    badge.textContent = `${cur._colCount} cols`;
    elt.appendChild(badge);
  } else if (cur.className === 'vr-hint-col' && cur._table) {
    const badge = document.createElement('span');
    badge.className = 'vr-hint-meta';
    badge.textContent = cur._table;
    elt.appendChild(badge);
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

window.vrDslSetSchema = function(json) {
  try { _schema = JSON.parse(json) || []; }
  catch(e) { console.warn('vrDslSetSchema: invalid JSON', e); }
};

window.vrDslInit = function(id, code) {
  if (typeof CodeMirror === 'undefined') {
    console.warn('vrDslInit: CodeMirror not loaded yet');
    return;
  }

  defineVrDslMode();
  injectTheme();

  const el = document.getElementById(id);
  if (!el) { console.warn('vrDslInit: element not found:', id); return; }

  if (_vrEditors[id]) { _vrEditors[id].toTextArea(); delete _vrEditors[id]; }

  el.innerHTML = '';
  const ta = document.createElement('textarea');
  ta.value = code || '';
  el.appendChild(ta);

  const cm = CodeMirror.fromTextArea(ta, {
    mode: 'vrdsl',
    theme: 'vinrouge',
    lineNumbers: true,
    tabSize: 2,
    indentWithTabs: false,
    lineWrapping: false,
    autofocus: true,
    extraKeys: {
      'Ctrl-Space': cm => {
        CodeMirror.showHint(cm, dslHint, {
          completeSingle: false,
          hint: dslHint,
          renderItem: renderHintItem,
        });
      },
      'Tab': cm => {
        if (cm.somethingSelected()) cm.indentSelection('add');
        else cm.replaceSelection('  ');
      },
    },
  });

  // Signature tooltip on every cursor/content change
  cm.on('cursorActivity', () => checkSignature(cm));
  cm.on('change', () => {
    checkSignature(cm);
    // Auto-trigger completion when user types a letter or dot
    const cur  = cm.getCursor();
    const line = cm.getLine(cur.line);
    const last = line[cur.ch - 1];
    if (/[A-Za-z_]/.test(last) || last === '.') {
      // Small delay so the character is committed first
      setTimeout(() => {
        if (!cm.state.completionActive) {
          CodeMirror.showHint(cm, dslHint, { completeSingle: false, renderItem: renderHintItem });
        }
      }, 80);
    }
  });

  cm.setSize('100%', '100%');
  _vrEditors[id] = cm;
};

window.vrDslGetValue = function(id) {
  return _vrEditors[id] ? _vrEditors[id].getValue() : '';
};

window.vrDslSetValue = function(id, code) {
  if (_vrEditors[id]) _vrEditors[id].setValue(code || '');
};

window.vrDslDestroy = function(id) {
  hideSig();
  if (!_vrEditors[id]) return;
  _vrEditors[id].toTextArea();
  delete _vrEditors[id];
};

window.vrDslRefresh = function(id) {
  if (_vrEditors[id]) _vrEditors[id].refresh();
};
