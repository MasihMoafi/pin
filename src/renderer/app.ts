// Pin's page. The tree lives in `board`; every change goes through change(), which saves the file and records one
// undo step (typing is grouped into one step per pause).
import { type Board, type Color, type Item, type Named, COLORS, parse, serialize } from './board.js';
import { icon, type IconName } from './icons.js';

const TINT: Record<Named, string> = { '🟢': '#b9ecc9', '🔵': '#b8d8f5', '🟣': '#e2d3f7', '🟠': '#ffe2bf', '🟡': '#fff0a8', '🔴': '#fdc6c7' };
const EDGE: Record<Named, string> = { '🟢': '#2f9e5b', '🔵': '#2f6fbe', '🟣': '#7e57c2', '🟠': '#e07b16', '🟡': '#d4a800', '🔴': '#d23b3b' };
const NAMES: Record<string, string> = {
  '🟢': 'Green', '🔵': 'Blue', '🟣': 'Purple', '🟠': 'Orange', '🟡': 'Yellow', '🔴': 'Red',
  '#ec4899': 'Pink', '#14b8a6': 'Teal', '#06b6d4': 'Cyan', '#84cc16': 'Lime', '#a16207': 'Brown', '#64748b': 'Gray',
};
// The palette: Rustidian's six named colors, then more as plain hex colors.
const PALETTE: Color[] = [...COLORS, '#ec4899', '#14b8a6', '#06b6d4', '#84cc16', '#a16207', '#64748b'];
const solid = (c: Color): string => (!c ? '' : c.startsWith('#') ? c : EDGE[c as Named]);
const tint = (c: Color): string => (!c ? '' : c.startsWith('#') ? c + '55' : TINT[c as Named]);

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;
const list = $<HTMLUListElement>('list');
const menu = $<HTMLDivElement>('menu');

interface FocusState { path: number[]; caret: number }
interface Snapshot { text: string; focus: FocusState | null }
type Timer = ReturnType<typeof setTimeout>;

let board: Board = { preamble: '', items: [] };
let shown: Snapshot | null = null;
let typing: Timer | null = null;
let writeTimer: Timer | undefined;
let dragged: Item | null = null;
let pinned = true;
const undoStack: Snapshot[] = [];
const redoStack: Snapshot[] = [];
const elOf = new Map<Item, HTMLElement>();
const itemOf = new WeakMap<Element, Item>();

// ---------- dates, shown the way Elpis Automations (after DeepSeek) shows them ----------
const pad = (n: number): string => String(n).padStart(2, '0');
const todayIso = (): string => { const d = new Date(); return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`; };
function whenText(value: string): string {
  const [day, time] = value.split(' ');
  const d = new Date(`${day}T${time || '00:00'}`);
  const opts: Intl.DateTimeFormatOptions = { weekday: 'short', day: 'numeric', month: 'short' };
  return time ? d.toLocaleString(undefined, { ...opts, hour: '2-digit', minute: '2-digit' }) : d.toLocaleDateString(undefined, opts);
}

// ---------- tree helpers ----------
function parentOf(item: Item, items: Item[] = board.items): Item[] | null {
  if (items.includes(item)) return items;
  for (const it of items) { const p = parentOf(item, it.children); if (p) return p; }
  return null;
}
function ownerOf(arr: Item[], items: Item[] = board.items, owner: Item | null = null): Item | null | undefined {
  if (items === arr) return owner;
  for (const it of items) { const o = ownerOf(arr, it.children, it); if (o !== undefined) return o; }
  return undefined;
}
function flat(items: Item[] = board.items, out: Item[] = []): Item[] { for (const it of items) { out.push(it); flat(it.children, out); } return out; }
function pathOf(item: Item, items: Item[] = board.items, path: number[] = []): number[] | null {
  for (let i = 0; i < items.length; i++) {
    if (items[i] === item) return [...path, i];
    const p = pathOf(item, items[i].children, [...path, i]); if (p) return p;
  }
  return null;
}
function itemAt(path: number[]): Item | null {
  let items = board.items; let it: Item | null = null;
  for (const i of path) { it = items[i] ?? null; if (!it) return null; items = it.children; }
  return it;
}
const contains = (a: Item, b: Item): boolean => a.children.includes(b) || a.children.some((c) => contains(c, b));
const blank = (color: Color, text = ''): Item => ({ text, color, date: null, done: false, children: [] });
const siblings = (item: Item): Item[] => parentOf(item) ?? board.items;

// ---------- caret ----------
function caretOf(el: HTMLElement): number {
  const s = getSelection();
  if (!s || !s.rangeCount || !s.focusNode || !el.contains(s.focusNode)) return (el.textContent ?? '').length;
  const r = document.createRange(); r.selectNodeContents(el); r.setEnd(s.focusNode, s.focusOffset);
  return r.toString().length;
}
function focusItem(item: Item, caret?: number): void {
  const el = elOf.get(item)?.querySelector<HTMLElement>('.title'); if (!el) return;
  el.focus();
  const s = getSelection(); if (!s) return;
  const r = document.createRange(); const node = el.firstChild as Text | null;
  if (node && caret != null) { r.setStart(node, Math.min(caret, node.length)); r.collapse(true); } else { r.selectNodeContents(el); r.collapse(false); }
  s.removeAllRanges(); s.addRange(r);
}
function focusState(): FocusState | null {
  const el = document.activeElement as HTMLElement | null; const item = el ? itemOf.get(el) : undefined;
  const path = item ? pathOf(item) : null;
  return el && path ? { path, caret: caretOf(el) } : null;
}
/** The caret sits on the first (up) or last (down) visual line of the title. */
function atEdge(el: HTMLElement, up: boolean): boolean {
  const s = getSelection(); if (!s || !s.rangeCount) return true;
  const r = s.getRangeAt(0).getClientRects()[0]; const box = el.getBoundingClientRect();
  if (!r) return true;
  return up ? r.top - box.top < r.height * 0.8 : box.bottom - r.bottom < r.height * 0.8;
}

// ---------- saving and undo ----------
const snapshot = (): Snapshot => ({ text: serialize(board), focus: focusState() });
function write(): void { clearTimeout(writeTimer); writeTimer = setTimeout(() => void host.write(serialize(board)), 250); }
function record(): void {
  if (typing) clearTimeout(typing); typing = null;
  const now = serialize(board);
  if (shown && now !== shown.text) { undoStack.push(shown); if (undoStack.length > 500) undoStack.shift(); redoStack.length = 0; }
  shown = snapshot();
}
/** Apply a structural change: save, redraw, put the caret back, then record an undo step. */
function change(fn: () => void, focus?: { item: Item; caret?: number } | null): void {
  if (typing) record();
  fn(); write(); render();
  if (focus) focusItem(focus.item, focus.caret);
  record();
}
function restore(state: Snapshot): void {
  board = parse(state.text); render(); shown = state;
  clearTimeout(writeTimer); void host.write(state.text);
  const it = state.focus && itemAt(state.focus.path); if (it && state.focus) focusItem(it, state.focus.caret);
}
function undo(): void { if (typing) record(); const s = undoStack.pop(); if (!s) return; redoStack.push(snapshot()); restore(s); }
function redo(): void { const s = redoStack.pop(); if (!s) return; undoStack.push(snapshot()); restore(s); }

// ---------- drawing ----------
function el<K extends keyof HTMLElementTagNameMap>(tag: K, className = ''): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag); if (className) e.className = className; return e;
}
function render(): void {
  elOf.clear();
  list.replaceChildren(...board.items.map(node));
}
function node(item: Item): HTMLLIElement {
  const li = el('li');
  const row = el('div', 'row' + (item.done ? ' done' : '') + (item.color ? ' tint' : ''));
  if (item.color) row.style.setProperty('--tint', tint(item.color));
  elOf.set(item, row);

  const handle = el('span', 'handle'); handle.innerHTML = icon('grip-vertical', 14); handle.title = 'Drag to reorder';
  // Only the handle starts a drag, so text in the title can be selected with the mouse.
  handle.onmousedown = () => { row.draggable = true; };
  row.ondragend = () => { row.draggable = false; };

  const check = el('input', 'check'); check.type = 'checkbox'; check.checked = item.done;
  check.onchange = () => change(() => { item.done = check.checked; });

  const title = el('div', 'title'); title.contentEditable = 'true'; title.spellcheck = false;
  title.textContent = item.text; itemOf.set(title, item);
  title.oninput = () => { item.text = title.textContent ?? ''; write(); if (typing) clearTimeout(typing); typing = setTimeout(record, 600); };
  title.onpaste = (e) => {
    e.preventDefault();
    document.execCommand('insertText', false, (e.clipboardData?.getData('text/plain') ?? '').replace(/\s*\n\s*/g, ' '));
  };
  title.onkeydown = (e) => keys(e, item, title);

  const when = el('span', 'when');
  if (item.date) {
    const date = item.date;
    when.textContent = whenText(date);
    const day = date.slice(0, 10);
    if (day === todayIso()) when.classList.add('today'); else if (day < todayIso() && !item.done) when.classList.add('late');
    when.title = 'Change date';
    when.onclick = () => dateMenu(when, item);
  }

  const dot = el('button', 'dot'); dot.title = 'Color';
  if (item.color) dot.style.background = solid(item.color);
  dot.onclick = () => colorMenu(dot, item);
  const addsub = el('button', 'addsub'); addsub.innerHTML = icon('plus', 20); addsub.title = 'Add subtask (Ctrl+T)';
  addsub.onclick = () => addSubtask(item);
  const del = el('button', 'del'); del.innerHTML = icon('trash-2', 17); del.title = 'Delete (Ctrl+Shift+Backspace)';
  del.onclick = () => remove(item);
  const more = el('button', 'more'); more.innerHTML = icon('ellipsis', 18); more.title = 'More';
  more.onclick = () => rowMenu(more, item);
  const hover = el('span', 'actions-hover'); hover.append(addsub, del, more);
  row.append(check, dot, title, when, handle, hover);

  row.ondragstart = (e) => { dragged = item; if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'; e.stopPropagation(); };
  row.ondragover = (e) => {
    if (!dragged || dragged === item || contains(dragged, item)) return;
    e.preventDefault(); const after = e.offsetY > row.offsetHeight / 2;
    row.classList.toggle('drop-after', after); row.classList.toggle('drop-before', !after);
  };
  row.ondragleave = () => row.classList.remove('drop-before', 'drop-after');
  row.ondrop = (e) => {
    e.preventDefault(); row.classList.remove('drop-before', 'drop-after');
    const moving = dragged; dragged = null;
    if (!moving || moving === item || contains(moving, item)) return;
    const after = e.offsetY > row.offsetHeight / 2;
    change(() => {
      const from = siblings(moving); from.splice(from.indexOf(moving), 1);
      const to = siblings(item); to.splice(to.indexOf(item) + (after ? 1 : 0), 0, moving);
    });
  };
  li.append(row);
  if (item.children.length) { const ul = el('ul'); ul.append(...item.children.map(node)); li.append(ul); }
  return li;
}

// ---------- keys ----------
function keys(e: KeyboardEvent, item: Item, title: HTMLElement): void {
  const p = siblings(item); const i = p.indexOf(item); const caret = caretOf(title); const text = title.textContent ?? '';
  const mod = e.ctrlKey || e.metaKey;
  if (mod && e.key.toLowerCase() === 't') { e.preventDefault(); addSubtask(item); }
  else if (mod && e.key.toLowerCase() === 'd') { e.preventDefault(); dateMenu(elOf.get(item) ?? title, item); }
  else if (mod && e.shiftKey && e.key === 'Backspace') {
    e.preventDefault(); const all = flat(); const before = all[all.indexOf(item) - 1];
    remove(item, before ? { item: before } : null);
  }
  else if (mod && e.key === 'Enter') { e.preventDefault(); change(() => { item.done = !item.done; }, { item, caret }); }
  else if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault(); const n = blank(item.color, text.slice(caret));
    change(() => { item.text = text.slice(0, caret); p.splice(i + 1, 0, n); }, { item: n, caret: 0 });
  }
  else if (e.key === 'Tab' && !e.shiftKey) {
    e.preventDefault(); if (i === 0) return;
    change(() => { p.splice(i, 1); p[i - 1].children.push(item); }, { item, caret });
  }
  else if (e.key === 'Tab' && e.shiftKey) {
    e.preventDefault(); const owner = ownerOf(p); if (!owner) return;
    change(() => { const gp = siblings(owner); p.splice(i, 1); gp.splice(gp.indexOf(owner) + 1, 0, item); }, { item, caret });
  }
  else if (e.key === 'Backspace' && !text && !item.children.length) {
    e.preventDefault(); const all = flat(); const before = all[all.indexOf(item) - 1];
    change(() => { p.splice(i, 1); }, before ? { item: before } : null);
  }
  else if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
    e.preventDefault(); const j = i + (e.key === 'ArrowUp' ? -1 : 1); if (j < 0 || j >= p.length) return;
    change(() => { p.splice(i, 1); p.splice(j, 0, item); }, { item, caret });
  }
  else if ((e.key === 'ArrowUp' && atEdge(title, true)) || (e.key === 'ArrowDown' && atEdge(title, false))) {
    const all = flat(); const next = all[all.indexOf(item) + (e.key === 'ArrowUp' ? -1 : 1)];
    if (next) { e.preventDefault(); focusItem(next, e.key === 'ArrowUp' ? undefined : 0); }
  }
}
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') { closeMenu(); return; }
  if (!(e.ctrlKey || e.metaKey) || (e.target as Element | null)?.closest?.('.pop')) return;
  const k = e.key.toLowerCase();
  if (k === 'z' && !e.shiftKey) { e.preventDefault(); undo(); }
  else if ((k === 'z' && e.shiftKey) || k === 'y') { e.preventDefault(); redo(); }
}, true);

// ---------- menus ----------
function openMenu(anchor: HTMLElement, build: (m: HTMLDivElement) => void): void {
  menu.replaceChildren(); build(menu); menu.hidden = false;
  const a = anchor.getBoundingClientRect(); const m = menu.getBoundingClientRect();
  menu.style.left = Math.max(6, Math.min(a.right - m.width, innerWidth - m.width - 6)) + 'px';
  menu.style.top = (a.bottom + m.height + 6 > innerHeight ? Math.max(6, a.top - m.height - 4) : a.bottom + 4) + 'px';
}
function closeMenu(): void { menu.hidden = true; }
document.addEventListener('mousedown', (e) => {
  const t = e.target as Element;
  if (!menu.hidden && !menu.contains(t) && !t.closest('.more, .when, .dot')) closeMenu();
});
function button(label: string, fn: () => void, cls = '', ic?: IconName): HTMLButtonElement {
  const b = el('button', cls); b.innerHTML = ic ? icon(ic, 16) : '';
  const span = el('span'); span.textContent = label; b.append(span);
  b.onclick = fn; return b;
}

function addSubtask(item: Item): void { const n = blank(item.color); change(() => { item.children.push(n); }, { item: n }); }
function swatches(item: Item): HTMLDivElement {
  const sw = el('div', 'swatches');
  const pick = (c: Color): void => { closeMenu(); change(() => { item.color = c; }); };
  for (const c of [...PALETTE, null]) {
    const b = el('button', item.color === c ? 'on' : ''); b.title = c ? NAMES[c] : 'No color';
    b.style.background = c ? solid(c) : '#fff';
    b.onclick = () => pick(c);
    sw.append(b);
  }
  const custom = el('label', 'custom'); custom.title = 'Any color'; custom.innerHTML = icon('palette', 16);
  const input = el('input'); input.type = 'color'; input.value = solid(item.color) || '#ec4899';
  input.onchange = () => pick(input.value.toLowerCase() as `#${string}`);
  custom.append(input); sw.append(custom);
  return sw;
}
function colorMenu(anchor: HTMLElement, item: Item): void { openMenu(anchor, (m) => m.append(swatches(item))); }
function rowMenu(anchor: HTMLElement, item: Item): void {
  openMenu(anchor, (m) => {
    const label = el('div', 'label'); label.textContent = 'Color';
    m.append(label, swatches(item), el('hr'));
    m.append(button(item.date ? 'Change date (Ctrl+D)' : 'Add date (Ctrl+D)', () => dateMenu(anchor, item), '', 'calendar'));
    m.append(button('Add subtask (Ctrl+T)', () => { closeMenu(); addSubtask(item); }, '', 'list-plus'));
    m.append(el('hr'));
    m.append(button('Delete (Ctrl+Shift+Backspace)', () => { closeMenu(); remove(item); }, '', 'trash-2'));
  });
}
/** Delete a task and its subtasks; Ctrl+Z brings them back. */
function remove(item: Item, focus?: { item: Item } | null): void {
  change(() => { const p = siblings(item); p.splice(p.indexOf(item), 1); }, focus);
}
function dateMenu(anchor: HTMLElement, item: Item): void {
  openMenu(anchor, (m) => {
    const label = el('div', 'label'); label.textContent = 'Date and time';
    const [day, time] = (item.date ?? '').split(' ');
    const fields = el('div', 'fields');
    const d = el('input'); d.type = 'date'; d.value = day || todayIso();
    const t = el('input'); t.type = 'time'; t.value = time || '';
    fields.append(d, t);
    const actions = el('div', 'actions');
    actions.append(
      button('Save', () => { closeMenu(); change(() => { item.date = d.value ? (t.value ? `${d.value} ${t.value}` : d.value) : null; }); }, 'primary'),
      button('Clear', () => { closeMenu(); change(() => { item.date = null; }); }),
    );
    m.append(label, fields, actions);
    d.focus();
  });
}

// ---------- header ----------
const addBtn = $<HTMLButtonElement>('add');
const pinBtn = $<HTMLButtonElement>('pin');
const closeBtn = $<HTMLButtonElement>('close');
addBtn.innerHTML = icon('plus', 18) + '<span>New task</span>';
addBtn.onclick = () => { const n = blank(board.items[0]?.color ?? '🟢'); change(() => { board.items.unshift(n); }, { item: n }); };
closeBtn.innerHTML = icon('x', 18);
closeBtn.onclick = () => host.close();
// The pin toggle, as Elpis draws a kept pin: filled in its own color while Pin floats on top, outline when not.
pinBtn.innerHTML = icon('pin', 18);
pinBtn.onclick = () => {
  pinned = !pinned; host.pin(pinned);
  pinBtn.classList.toggle('on', pinned); pinBtn.setAttribute('aria-pressed', String(pinned));
  pinBtn.title = pinned ? 'Pinned on top: click to unpin' : 'Not on top: click to pin';
};

host.onChanged((text) => {
  if ((document.activeElement as HTMLElement | null)?.isContentEditable || text === serialize(board)) return;
  board = parse(text); render(); record();
});
void host.read().then((text) => { board = parse(text); render(); shown = snapshot(); });
