// PRIORITIES.md <-> a task tree. The file stays plain Markdown that Rustidian shows in color:
//   - [ ] ==🟢Swarm · today's work== 📅 2026-11-07 09:30
//     - [x] a subtask
// Line order is priority; two spaces of indent make a subtask. Rustidian's six named colors use its ==🟢…==
// markers; any other color is a portable <mark style="background: #rrggbb;"> highlight, which Rustidian also shows.

export const COLORS = ['🟢', '🔵', '🟣', '🟠', '🟡', '🔴'] as const;
export type Named = (typeof COLORS)[number];
/** A named Rustidian color, a `#rrggbb` hex color, or none. */
export type Color = Named | `#${string}` | null;

export interface Item {
  text: string;
  color: Color;
  /** `YYYY-MM-DD`, optionally followed by ` HH:MM`. */
  date: string | null;
  done: boolean;
  children: Item[];
}
export interface Board {
  preamble: string;
  items: Item[];
}

/** Keep unfinished siblings in priority order, with completed tasks and their branches below them. */
export function completedLast(items: Item[]): void {
  items.sort((a, b) => Number(a.done) - Number(b.done));
  for (const item of items) completedLast(item.children);
}

const ITEM = /^(\s*)[-*] (?:\[([ xX])\] )?(.*)$/;
const DATE = /\s*📅\s?(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}))?\s*$/;
const NAMED = /^==(\S{1,2}?)(.*)==$/u;
const HEX = /^<mark style="background: (#[0-9a-fA-F]{6})(?:[0-9a-fA-F]{2})?;">(.*)<\/mark>$/;

const isNamed = (s: string): s is Named => (COLORS as readonly string[]).includes(s);

function parseBody(raw: string): Pick<Item, 'text' | 'color' | 'date'> {
  let body = raw;
  let date: string | null = null;
  const d = body.match(DATE);
  if (d && d.index !== undefined) {
    date = d[2] ? `${d[1]} ${d[2]}` : d[1];
    body = body.slice(0, d.index);
  }
  let color: Color = null;
  const m = body.match(NAMED);
  const hex = body.match(HEX);
  if (m && isNamed(m[1])) {
    color = m[1];
    body = m[2];
  } else if (hex) {
    color = hex[1].toLowerCase() as `#${string}`;
    body = hex[2];
  }
  return { text: body.trim(), color, date };
}

export function parse(markdown: string): Board {
  const preamble: string[] = [];
  const roots: Item[] = [];
  const stack: { depth: number; children: Item[] }[] = [{ depth: -1, children: roots }];
  let inList = false;
  for (const line of markdown.split('\n')) {
    const m = line.match(ITEM);
    if (!m) {
      if (!inList && line.trim()) preamble.push(line);
      continue;
    }
    inList = true;
    const depth = Math.floor(m[1].replace(/\t/g, '  ').length / 2);
    const item: Item = { ...parseBody(m[3]), done: m[2] === 'x' || m[2] === 'X', children: [] };
    while (stack.length > 1 && stack[stack.length - 1].depth >= depth) stack.pop();
    stack[stack.length - 1].children.push(item);
    stack.push({ depth, children: item.children });
  }
  return { preamble: preamble.join('\n'), items: roots };
}

export function serialize(board: Board): string {
  const lines: string[] = [];
  const walk = (items: Item[], depth: number): void => {
    for (const it of items) {
      let body = it.text;
      if (it.color?.startsWith('#')) body = `<mark style="background: ${it.color};">${body}</mark>`;
      else if (it.color) body = `==${it.color}${body}==`;
      if (it.date) body += ` 📅 ${it.date}`;
      lines.push(`${'  '.repeat(depth)}- [${it.done ? 'x' : ' '}] ${body}`);
      walk(it.children, depth + 1);
    }
  };
  walk(board.items, 0);
  return (board.preamble ? board.preamble + '\n\n' : '') + lines.join('\n') + '\n';
}
