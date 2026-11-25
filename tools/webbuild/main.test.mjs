// web/main.js against a stub engine and a small in-memory IndexedDB:
// node --test tools/webbuild/main.test.mjs
import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, copyFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dir = mkdtempSync(join(tmpdir(), 'qsp-web-'));
process.on('exit', () => rmSync(dir, { recursive: true, force: true }));
copyFileSync(join(here, '../../web/main.js'), join(dir, 'main.mjs'));
// the engine: calls go to globalThis.engine
writeFileSync(join(dir, 'quake_source_port.js'), ['start', 'page_hidden', 'vfs_mount', 'vfs_modified', 'vfs_take_dirty', 'vfs_read']
  .map((f) => `export const ${f} = (...a) => engine.${f}(...a);`).join('\n') + '\nexport default () => engine.init();\n');
writeFileSync(join(dir, 'package.json'), '{"type":"module"}');

const tick = () => new Promise((ok) => setImmediate(ok));
async function settle() { for (let i = 0; i < 20; i++) await tick(); }

// Requests run in order; a transaction commits whole or not at all.
function fakeIndexedDB(stores) {
  const state = { failPut: () => false, transactions: [] };
  const openTx = (name) => {
    const ops = [];
    const t = { error: null };
    const keys = [];
    state.transactions.push(keys);
    const request = (run) => { const r = {}; ops.push(() => run(r)); return r; };
    t.abort = () => { ops.length = 0; t.aborted = true; };
    t.objectStore = () => ({
      put: (v, k) => { keys.push(k); return request(() => {
        if (state.failPut(k)) throw Object.assign(new Error('no space'), { name: 'QuotaExceededError' });
        t.copy.set(k, v);
      }); },
      delete: (k) => { keys.push(k); return request(() => t.copy.delete(k)); },
      clear: () => request(() => t.copy.clear()),
      openCursor: () => request((r) => {
        for (const [key, value] of [...t.copy]) r.onsuccess({ target: r, ...(r.result = { key, value, continue() {} }) });
        r.result = null;
        r.onsuccess();
      }),
    });
    state.queue = (state.queue ?? Promise.resolve()).then(tick).then(() => {
      t.copy = new Map(stores[name]);
      try {
        for (const op of ops) op();
        stores[name] = t.copy;
        t.oncomplete?.();
      } catch (e) {
        t.error = e;
        t.onabort?.({ target: t });
      }
    });
    return t;
  };
  state.open = () => {
    const r = { result: { transaction: openTx } };
    setImmediate(() => r.onsuccess());
    return r;
  };
  return state;
}

let page;
let runs = 0;
async function load({ game = [['/quake/id1/pak0.pak', new Uint8Array([1])]], user = [] } = {}) {
  const elements = {};
  const element = (id) => (elements[id] ??= {
    id, hidden: false, textContent: '', style: {}, dataset: {}, listeners: {}, classList: { add() {}, remove() {} },
    addEventListener(n, f) { this.listeners[n] = f; }, setAttribute() {}, focus() {},
  });
  for (const id of ['status', 'play', 'forget', 'pick', 'drop', 'setup', 'quake']) element(id);
  const stores = { game: new Map(game), user: new Map(user) };
  const files = new Map();
  const dirty = new Set();
  const calls = { hidden: 0, started: 0, persist: 0 };
  let now = 1000000;
  page = {
    elements, stores, files, dirty, calls, window: {}, document: {}, interval: null,
    idb: fakeIndexedDB(stores),
    write(path, bytes, modified = now) { files.set(path, { data: bytes, modified }); dirty.add(path); },
    remove(path) { files.delete(path); dirty.add(path); },
    advance(ms) { now += ms; },
    async tick(ms = 2000) { now += ms; page.interval(); await settle(); },
    async leave() { page.window.pagehide(); await settle(); },
    get warning() { const b = elements['storage-warning']; return b && !b.hidden ? b.textContent : ''; },
  };
  const set = (k, v) => Object.defineProperty(globalThis, k, { value: v, configurable: true, writable: true });
  set('engine', {
    init: async () => {},
    start: async () => { calls.started++; },
    page_hidden: () => { calls.hidden++; page.onHidden?.(); },
    vfs_mount: (p, data, modified) => files.set(p, { data, modified: modified ?? now }),
    vfs_modified: (p) => files.get(p)?.modified,
    vfs_take_dirty: () => { const d = [...dirty]; dirty.clear(); return d; },
    vfs_read: (p) => files.get(p)?.data,
  });
  set('document', {
    hidden: false,
    getElementById: (id) => elements[id] ?? null,
    createElement: () => ({ style: {}, setAttribute() {}, textContent: '', hidden: false }),
    body: { appendChild: (e) => { elements[e.id] = e; } },
    addEventListener: (n, f) => { page.document[n] = f; },
  });
  set('navigator', { gpu: {}, storage: { persist: async () => { calls.persist++; return true; } } });
  set('location', { search: '' });
  set('indexedDB', page.idb);
  set('addEventListener', (n, f) => { page.window[n] = f; });
  set('setInterval', (f) => { page.interval = f; });
  page.realNow ??= Date.now;
  Date.now = () => now;
  console.error = () => {};
  await import(`${pathToFileURL(join(dir, 'main.mjs'))}?${++runs}`);
  await settle();
  return page;
}
async function play(o) {
  await load(o);
  page.elements.play.listeners.click();
  await settle();
  assert.equal(page.calls.started, 1, page.elements.status.textContent);
  return page;
}
const bytes = (...b) => new Uint8Array(b);
beforeEach(() => { page = null; });

test('written files are stored with their times and come back with them', async () => {
  await play();
  page.write('/quake-user/id1/a.sav', bytes(1), 111);
  await page.tick();
  assert.deepEqual(page.stores.user.get('/quake-user/id1/a.sav'), { data: bytes(1), modified: 111 });
  assert.equal(page.calls.persist, 1);
  const user = [...page.stores.user];
  await play({ user });
  assert.deepEqual(page.files.get('/quake-user/id1/a.sav'), { data: bytes(1), modified: 111 });
  page.remove('/quake-user/id1/a.sav');
  await page.tick();
  assert.equal(page.stores.user.size, 0);
});

test('files stored by earlier versions get a time that stays', async () => {
  await play({ user: [['/quake-user/id1/a.sav', bytes(1)]] });
  const first = page.files.get('/quake-user/id1/a.sav').modified;
  await page.tick();
  assert.deepEqual(page.stores.user.get('/quake-user/id1/a.sav'), { data: bytes(1), modified: first });
});

test('a write that fails is retried and told', async () => {
  await play();
  page.idb.failPut = () => true;
  page.write('/quake-user/id1/a.sav', bytes(1));
  await page.tick();
  assert.equal(page.stores.user.size, 0);
  assert.match(page.warning, /could not be stored/);
  page.idb.failPut = () => false;
  await page.tick();
  assert.equal(page.stores.user.size, 0, 'not tried again at once');
  await page.tick(10000);
  assert.equal(page.stores.user.size, 1);
  assert.equal(page.warning, '');
});

test('one file too big to store does not take the others with it', async () => {
  await play();
  page.idb.failPut = (k) => k.endsWith('.dem');
  page.write('/quake-user/id1/big.dem', new Uint8Array(5 << 20));
  page.write('/quake-user/id1/a.sav', bytes(1));
  await page.tick();
  assert.deepEqual([...page.stores.user.keys()], ['/quake-user/id1/a.sav']);
  assert.notEqual(page.warning, '');
  // saves made meanwhile are tried too
  page.write('/quake-user/id1/b.sav', bytes(2));
  await page.tick(10000);
  assert.ok(page.stores.user.has('/quake-user/id1/b.sav'));
  page.idb.failPut = () => false;
  await page.tick(10000);
  assert.equal(page.stores.user.size, 3);
});

test('a rename is stored whole', async () => {
  await play({ user: [['/quake-user/id1/a.sav', { data: bytes(1), modified: 5 }]] });
  page.idb.failPut = () => true;
  page.files.delete('/quake-user/id1/a.sav');
  page.dirty.add('/quake-user/id1/a.sav');
  page.write('/quake-user/id1/b.sav', bytes(1), 5);
  await page.tick();
  assert.deepEqual([...page.stores.user.keys()], ['/quake-user/id1/a.sav'], 'the old name stays until the new one is in');
  page.idb.failPut = () => false;
  await page.tick(10000);
  assert.deepEqual([...page.stores.user.keys()], ['/quake-user/id1/b.sav']);
});

test('a change made while its file is being stored is stored too', async () => {
  await play();
  page.write('/quake-user/id1/config.cfg', bytes(1));
  page.interval();
  page.write('/quake-user/id1/config.cfg', bytes(2));
  await settle();
  await page.tick();
  assert.deepEqual(page.stores.user.get('/quake-user/id1/config.cfg').data, bytes(2));
  const n = page.idb.transactions.length;
  await page.tick();
  assert.equal(page.idb.transactions.length, n, 'nothing left to store');
});

test('a change whose store fails stays pending when an earlier one gets through', async () => {
  await play();
  const path = '/quake-user/id1/config.cfg';
  let puts = 0;
  page.idb.failPut = () => ++puts === 2;
  page.write(path, bytes(1));
  page.interval();
  page.write(path, bytes(2));
  page.interval();
  await settle();
  assert.deepEqual(page.stores.user.get(path).data, bytes(1));
  await page.tick(10000);
  assert.deepEqual(page.stores.user.get(path).data, bytes(2));
});

test('a growing demo is not stored again every time', async () => {
  await play();
  let stored = 0;
  for (let i = 0; i < 30; i++) {
    page.write('/quake-user/id1/auto.dem', new Uint8Array((5 << 20) + i));
    const n = page.idb.transactions.length;
    await page.tick();
    stored += page.idb.transactions.length - n;
  }
  assert.ok(stored >= 2 && stored <= 3, `${stored} times in a minute`);
  await page.leave();
  assert.equal(page.stores.user.get('/quake-user/id1/auto.dem').data.length, (5 << 20) + 29, 'the rest when the page goes');
});

test('leaving or hiding the page stores the settings', async () => {
  await play();
  page.onHidden = () => page.write('/quake-user/id1/config.cfg', bytes(7));
  await page.leave();
  assert.equal(page.calls.hidden, 1);
  assert.deepEqual(page.stores.user.get('/quake-user/id1/config.cfg').data, bytes(7));
  document.hidden = true;
  page.document.visibilitychange();
  await settle();
  assert.equal(page.calls.hidden, 2);
  document.hidden = false;
  page.document.visibilitychange();
  assert.equal(page.calls.hidden, 2);
});

test('closing the page asks only while somebody plays', async () => {
  await play();
  let asked = 0;
  const close = () => page.window.beforeunload({ preventDefault() { asked++; } });
  close();
  assert.equal(asked, 0);
  document.pointerLockElement = {};
  close();
  assert.equal(asked, 1);
});

test('only user files are kept and mounted', async () => {
  await play({ user: [['/quake/id1/pak0.pak', { data: bytes(66), modified: 1 }], ['/quake-user/id1/a.sav', { data: bytes(1), modified: 1 }]] });
  assert.deepEqual(page.files.get('/quake/id1/pak0.pak').data, bytes(1), 'the game data stored by the user');
  page.write('/quake/id1/pak0.pak', bytes(67));
  await page.leave();
  assert.deepEqual(page.stores.user.get('/quake/id1/pak0.pak').data, bytes(66), 'never written');
});

test('upper-case game dirs are mounted in lower case', async () => {
  await play({ game: [['/quake/ID1/PAK0.PAK', bytes(1)]] }); // stored by an earlier version
  assert.ok(page.files.has('/quake/id1/PAK0.PAK'));
  await load({ game: [] });
  const file = (p) => ({ webkitRelativePath: p, name: p, arrayBuffer: async () => new ArrayBuffer(4) });
  await page.elements.pick.listeners.change({ target: { files: [file('Quake/ID1/PAK0.PAK'), file('Quake/HIPNOTIC/PAK0.PAK')] } });
  await settle();
  assert.deepEqual([...page.stores.game.keys()], ['/quake/id1/PAK0.PAK', '/quake/hipnotic/PAK0.PAK']);
  assert.equal(page.calls.started, 1);
});

test('errors while storing or starting are shown', async () => {
  await load({ game: [] });
  page.idb.failPut = () => true;
  const file = { webkitRelativePath: 'id1/pak0.pak', name: 'pak0.pak', arrayBuffer: async () => new ArrayBuffer(4) };
  await page.elements.pick.listeners.change({ target: { files: [file] } });
  assert.match(page.elements.status.textContent, /could not store id1\/pak0.pak \(no space\)/);
  await load();
  engine.init = async () => { throw new Error('no wasm'); };
  page.elements.play.listeners.click();
  await settle();
  assert.match(page.elements.status.textContent, /Failed to start: no wasm/);
  assert.equal(page.elements.setup.hidden, false);
});
