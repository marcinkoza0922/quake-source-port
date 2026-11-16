// Browser front-end: collects the user's Quake data, keeps it (and saves,
// configs) in IndexedDB, mounts it into the engine's VFS and starts the game.
import init, { start, vfs_mount, vfs_take_dirty, vfs_read } from './quake_source_port.js';

const GAME_ROOT = '/quake/';
const $ = (id) => document.getElementById(id);
const status = (s) => { $('status').textContent = s; };

// ---- IndexedDB: store "game" (path -> bytes under /quake), "user" (/quake-user) ----
function openDb() {
  return new Promise((ok, fail) => {
    const r = indexedDB.open('quake', 1);
    r.onupgradeneeded = () => { r.result.createObjectStore('game'); r.result.createObjectStore('user'); };
    r.onsuccess = () => ok(r.result);
    r.onerror = () => fail(r.error);
  });
}
const done = (t) => new Promise((ok, fail) => { t.oncomplete = ok; t.onerror = t.onabort = () => fail(t.error); });
async function putAll(db, store, entries) {
  const t = db.transaction(store, 'readwrite');
  for (const [k, v] of entries) v === undefined ? t.objectStore(store).delete(k) : t.objectStore(store).put(v, k);
  await done(t);
}
function getAll(db, store) {
  return new Promise((ok, fail) => {
    const out = [];
    const r = db.transaction(store).objectStore(store).openCursor();
    r.onsuccess = () => { const c = r.result; if (c) { out.push([c.key, c.value]); c.continue(); } else ok(out); };
    r.onerror = () => fail(r.error);
  });
}
async function clearStore(db, store) { const t = db.transaction(store, 'readwrite'); t.objectStore(store).clear(); await done(t); }

// ---- collecting files ----
// Picked/dropped paths look like "Quake/id1/pak0.pak", "id1/pak0.pak" or "pak0.pak".
// Drop a leading folder that is not a game dir; loose .pak files go into id1/.
function gamePath(rel) {
  const parts = rel.split('/').filter(Boolean);
  if (parts.length === 1) return /\.pak$/i.test(parts[0]) ? `id1/${parts[0]}` : null;
  if (parts.length > 1 && !/^id1$/i.test(parts[0]) && !looksLikeGameDir(parts)) parts.shift();
  return parts.join('/');
}
const looksLikeGameDir = (parts) => parts.length >= 2 && /\.(pak|pk3|dat)$/i.test(parts[1]);

async function filesFromEntry(entry, prefix, out) {
  if (entry.isFile) {
    out.push([prefix + entry.name, await new Promise((ok, fail) => entry.file(ok, fail))]);
  } else if (entry.isDirectory) {
    const reader = entry.createReader();
    for (;;) {
      const batch = await new Promise((ok, fail) => reader.readEntries(ok, fail));
      if (!batch.length) break;
      for (const e of batch) await filesFromEntry(e, `${prefix}${entry.name}/`, out);
    }
  }
}

async function storeFiles(db, list) {
  let n = 0, bytes = 0;
  for (const [rel, file] of list) {
    const p = gamePath(rel);
    if (!p) continue;
    const data = new Uint8Array(await file.arrayBuffer());
    await putAll(db, 'game', [[GAME_ROOT + p, data]]); // one per transaction keeps memory bounded
    n++; bytes += data.length;
    status(`Stored ${n} files (${(bytes / 1048576).toFixed(1)} MB)…`);
  }
  return n;
}

// ---- persistence of files the engine writes ----
async function persistDirty(db) {
  const paths = vfs_take_dirty();
  if (!paths.length) return;
  await putAll(db, 'user', paths.map((p) => [p, vfs_read(p)])); // undefined -> removed
}

async function launch(db) {
  status('Loading…');
  await init();
  const game = await getAll(db, 'game');
  if (!game.some(([k]) => /\/id1\/pak0\.pak$/i.test(k))) {
    status('id1/pak0.pak not found in the stored data.');
    return;
  }
  for (const [k, v] of game) vfs_mount(k, v);
  for (const [k, v] of await getAll(db, 'user')) vfs_mount(k, v);
  const args = (new URLSearchParams(location.search).get('args') || '').split(' ').filter(Boolean);
  $('setup').hidden = true;
  const canvas = $('quake');
  canvas.hidden = false;
  canvas.focus();
  setInterval(() => persistDirty(db).catch(console.error), 2000);
  const flush = () => persistDirty(db).catch(console.error);
  addEventListener('pagehide', flush);
  document.addEventListener('visibilitychange', () => { if (document.hidden) flush(); });
  try {
    await start(args);
  } catch (e) {
    $('setup').hidden = false;
    canvas.hidden = true;
    status(`Failed to start: ${e}`);
    console.error(e);
  }
}

async function main() {
  if (!navigator.gpu) status('This browser has no WebGPU; the game cannot start.');
  const db = await openDb();
  const refresh = async () => {
    const have = (await getAll(db, 'game')).length > 0;
    $('play').hidden = !have;
    $('forget').hidden = !have;
    if (have) status('Game data found in this browser. Press Play.');
  };
  $('pick').addEventListener('change', async (ev) => {
    const list = [...ev.target.files].map((f) => [f.webkitRelativePath || f.name, f]);
    if (await storeFiles(db, list)) await launch(db); else status('No usable files picked.');
  });
  const drop = $('drop');
  drop.addEventListener('dragover', (e) => { e.preventDefault(); drop.classList.add('over'); });
  drop.addEventListener('dragleave', () => drop.classList.remove('over'));
  drop.addEventListener('drop', async (e) => {
    e.preventDefault();
    drop.classList.remove('over');
    const entries = [...e.dataTransfer.items].map((i) => i.webkitGetAsEntry()).filter(Boolean);
    const list = [];
    for (const en of entries) await filesFromEntry(en, '', list);
    if (await storeFiles(db, list)) await launch(db); else status('No usable files dropped.');
  });
  $('play').addEventListener('click', () => launch(db));
  $('forget').addEventListener('click', async () => {
    await clearStore(db, 'game');
    status('Stored game data removed (saves kept).');
    await refresh();
  });
  await refresh();
}

main().catch((e) => { status(`Error: ${e}`); console.error(e); });
