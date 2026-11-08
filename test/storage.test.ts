import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { readList, saveList } from '../src/storage.ts';

function temporary(t: test.TestContext): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pin-storage-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return path.join(dir, 'Pin.md');
}

test('a list saves atomically without using a shared temporary name', (t) => {
  const file = temporary(t);
  assert.equal(readList(file), null);
  const other = file + '.tmp';
  fs.writeFileSync(other, 'another program');
  assert.deepEqual(saveList(file, '- [ ] First\n', null), { saved: true });
  assert.equal(readList(file), '- [ ] First\n');
  assert.equal(fs.readFileSync(other, 'utf8'), 'another program');
  assert.deepEqual(fs.readdirSync(path.dirname(file)).sort(), ['Pin.md', 'Pin.md.tmp']);
});

test('an external replacement is returned instead of overwritten', (t) => {
  const file = temporary(t);
  fs.writeFileSync(file, '- [ ] Original\n');
  const original = readList(file);
  const replacement = file + '.sync';
  fs.writeFileSync(replacement, '- [ ] From another device\n');
  fs.renameSync(replacement, file);
  assert.deepEqual(saveList(file, '- [ ] Local edit\n', original), {
    saved: false, disk: '- [ ] From another device\n',
  });
  assert.equal(readList(file), '- [ ] From another device\n');
});

test('saving through a symlink keeps the link and updates its target', (t) => {
  const file = temporary(t);
  const target = path.join(path.dirname(file), 'synced.md');
  fs.writeFileSync(target, '- [ ] Before\n', { mode: 0o640 });
  fs.symlinkSync('synced.md', file);
  assert.deepEqual(saveList(file, '- [ ] After\n', readList(file)), { saved: true });
  assert.equal(fs.lstatSync(file).isSymbolicLink(), true);
  assert.equal(fs.readFileSync(target, 'utf8'), '- [ ] After\n');
  assert.equal(fs.statSync(target).mode & 0o777, 0o640);
});

test('a file replaced by a directory raises an error instead of reading as empty', (t) => {
  const file = temporary(t);
  fs.mkdirSync(file);
  assert.throws(() => readList(file));
});
