import assert from 'node:assert';
import test from 'node:test';
import { parse, serialize } from '../src/renderer/board.ts';

test('colors, dates, done and subtasks survive a round trip', () => {
  const md = 'Intro line.\n\n- [ ] ==🟢Swarm · today== 📅 2026-11-07 09:30\n  - [x] sub one\n  - [ ] ==🔴sub two==\n- [ ] <mark style="background: #ec4899;">Pink</mark> 📅 2026-11-08\n';
  const board = parse(md);
  assert.equal(board.items[0].color, '🟢');
  assert.equal(board.items[0].date, '2026-11-07 09:30');
  assert.equal(board.items[0].text, 'Swarm · today');
  assert.equal(board.items[0].children.length, 2);
  assert.equal(board.items[0].children[0].done, true);
  assert.equal(board.items[0].children[1].color, '🔴');
  assert.equal(board.items[1].color, '#ec4899');
  assert.equal(serialize(board), md);
});

test('old lines without checkboxes still read, and an unknown marker is kept as text', () => {
  const board = parse('- ==🟢Elpis · x==\n- ==Q not a color==\n');
  assert.equal(board.items[0].color, '🟢');
  assert.equal(board.items[0].done, false);
  assert.equal(board.items[1].color, null);
  assert.equal(board.items[1].text, '==Q not a color==');
});
