import assert from 'node:assert';
import test from 'node:test';
import { completedLast, parse, serialize } from '../src/renderer/board.ts';

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

test('completed tasks sort below unfinished siblings without changing priority or parentage', () => {
  const board = parse('Intro.\n\n- [x] Finished first\n  - [ ] Its child\n- [ ] ==🟢Priority one== 📅 2026-11-09\n  - [x] Done child\n    - [ ] Grandchild\n  - [ ] Active child\n- [x] Finished second\n- [ ] Priority two\n');
  const parent = board.items[1];
  const finished = board.items[0];
  completedLast(board.items);
  assert.deepEqual(board.items.map(item => item.text), ['Priority one', 'Priority two', 'Finished first', 'Finished second']);
  assert.equal(board.items[0], parent);
  assert.equal(board.items[2], finished);
  assert.deepEqual(parent.children.map(item => item.text), ['Active child', 'Done child']);
  assert.equal(parent.children[1].children[0].text, 'Grandchild');
  assert.equal(finished.children[0].text, 'Its child');
  assert.equal(parent.color, '🟢');
  assert.equal(parent.date, '2026-11-09');
  const text = serialize(board);
  completedLast(board.items);
  assert.equal(serialize(board), text);
});

test('all twelve task colors survive a file round trip', () => {
  const md = [
    '- [ ] ==🟢Green==', '- [ ] ==🔵Blue==', '- [ ] ==🟣Purple==',
    '- [ ] ==🟠Orange==', '- [ ] ==🟡Yellow==', '- [ ] ==🔴Red==',
    ...['#ec4899', '#14b8a6', '#06b6d4', '#84cc16', '#a16207', '#64748b']
      .map((color) => `- [ ] <mark style="background: ${color};">Task</mark>`),
  ].join('\n') + '\n';
  const board = parse(md);
  assert.equal(board.items.length, 12);
  assert.equal(serialize(board), md);
});
