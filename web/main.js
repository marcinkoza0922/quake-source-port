// Browser front-end: collects the user's Quake data, keeps it (and saves,
// configs) in IndexedDB, mounts it into the engine's VFS and starts the game.
// A page may also offer the shareware episode: a `#shareware` button with a
// `data-url` pointing at the shareware id1/pak0.pak (off when absent).
import init, { start, page_hidden, vfs_mount, vfs_modified, vfs_take_dirty, vfs_read } from './quake_source_port.js';

const GAME_ROOT = '/quake/';
const USER_ROOT = '/quake-user/'; // all that is kept of what the engine writes
const $ = (id) => document.getElementById(id);
const status = (s) => { $('status').textContent = s; };

// Shown over the game; empty hides it.
function warn(text) {
  let b = $('storage-warning');
  if (!b) {
    if (!text) return;
    b = document.createElement('div');
    b.id = 'storage-warning';
    b.setAttribute('role', 'alert');
    b.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:10;padding:6px 12px;'
      + 'background:#7a1f1f;color:#fff;font:14px system-ui,sans-serif;text-align:center;pointer-events:none';
    document.body.appendChild(b);
  }
  b.textContent = text;
  b.hidden = !text;
}

// ---- IndexedDB: store "game" (path -> bytes under /quake), "user" (path -> {data, modified}) ----
function openDb() {
  return new Promise((ok, fail) => {
    const r = indexedDB.open('quake', 1);
    r.onupgradeneeded = () => { r.result.createObjectStore('game'); r.result.createObjectStore('user'); };
    r.onsuccess = () => ok(r.result);
    r.onerror = () => fail(r.error);
  });
}
const done = (t) => new Promise((ok, fail) => {
  t.oncomplete = ok;
  t.onerror = t.onabort = (e) => fail(t.error || e?.target?.error || new Error('storage failed'));
});
async function putAll(db, store, entries) {
  const t = db.transaction(store, 'readwrite');
  const end = done(t);
  try {
    for (const [k, v] of entries) v === undefined ? t.objectStore(store).delete(k) : t.objectStore(store).put(v, k);
  } catch (e) {
    end.catch(() => {});
    try { t.abort(); } catch { /* already over */ }
    throw e;
  }
  await end;
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
  return lowerDir(parts.join('/'));
}
// The engine asks for game dirs in lower case (CDs have ID1/); it finds the
// files in them in any case.
const lowerDir = (p) => p.replace(/^[^/]+(?=\/)/, (d) => d.toLowerCase());
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
    try {
      await putAll(db, 'game', [[GAME_ROOT + p, data]]); // one per transaction keeps memory bounded
    } catch (e) {
      throw new Error(`could not store ${p} (${e.message || e}). The browser may be out of space.`);
    }
    n++; bytes += data.length;
    status(`Stored ${n} files (${(bytes / 1048576).toFixed(1)} MB)…`);
  }
  return n;
}

// ---- optional shareware download ----
async function fetchShareware(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`download failed (${r.status})`);
  // Content-Length is the compressed size when the server gzips; only trust it uncompressed.
  const total = r.headers.get('content-encoding') ? 0 : Number(r.headers.get('content-length')) || 0;
  const bar = $('progress');
  if (bar) { bar.hidden = false; bar.removeAttribute('value'); }
  const reader = r.body.getReader();
  const chunks = [];
  let got = 0;
  for (;;) {
    const { done: end, value } = await reader.read();
    if (end) break;
    chunks.push(value);
    got += value.length;
    if (total && bar) bar.value = got / total;
    status(`Downloading shareware data… ${(got / 1048576).toFixed(1)} MB`);
  }
  if (bar) bar.hidden = true;
  const data = new Uint8Array(got);
  let at = 0;
  for (const c of chunks) { data.set(c, at); at += c.length; }
  return data;
}

// ---- persistence of files the engine writes ----
const BIG = 4 << 20; // stored on their own (demos)...
const BIG_EVERY = 30000; // ...and while they grow only this often (ms)
const RETRY = 10000;
const pending = new Map(); // path -> number of its latest change, until that is stored
const storing = new Map(); // path -> number of the change being stored
const bigStored = new Map(); // path -> when a big file was last stored
let changes = 0;
let failedAt = 0;

const changed = (path) => pending.set(path, ++changes);

async function store(db, paths) {
  const entries = paths.map(([path]) => {
    const data = vfs_read(path);
    return [path, data && { data, modified: vfs_modified(path) }]; // undefined -> removed
  });
  for (const [path, n] of paths) storing.set(path, n);
  try {
    await putAll(db, 'user', entries);
    for (const [path, n] of paths) if (pending.get(path) === n) pending.delete(path);
  } finally {
    for (const [path, n] of paths) if (storing.get(path) === n) storing.delete(path);
  }
}

// Stores what changed. Small files go in one transaction, so renames are kept
// whole; what fails stays pending. `leaving`: everything, now.
async function persistDirty(db, leaving) {
  for (const p of vfs_take_dirty()) if (p.startsWith(USER_ROOT)) changed(p);
  const now = Date.now();
  if (failedAt && !leaving && now - failedAt < RETRY) return;
  const small = [];
  const jobs = [];
  for (const job of pending) {
    const [path, n] = job;
    if (storing.get(path) === n) continue;
    if (!leaving && now - (bigStored.get(path) ?? -Infinity) < BIG_EVERY) continue;
    const size = vfs_read(path)?.length ?? 0;
    if (size > BIG) {
      bigStored.set(path, now);
      jobs.push([job]);
    } else {
      bigStored.delete(path);
      small.push(job);
    }
  }
  if (small.length) jobs.unshift(small);
  if (!jobs.length) return;
  // all started before the first await: the page may be going away
  const results = await Promise.allSettled(jobs.map((j) => store(db.handle, j)));
  const failed = results.find((r) => r.status === 'rejected');
  if (!failed) {
    failedAt = 0;
    warn('');
    return;
  }
  failedAt = Date.now();
  for (const [i, r] of results.entries()) if (r.status === 'rejected') for (const [path] of jobs[i]) bigStored.delete(path);
  console.error(failed.reason);
  warn(`Saves and settings could not be stored in this browser (${failed.reason?.message || failed.reason}). `
    + 'They are lost when this page closes. Trying again…');
  if (failed.reason?.name === 'InvalidStateError') db.handle = await openDb(); // the connection was closed
}

async function launch(db) {
  const canvas = $('quake');
  try {
    status('Loading…');
    await init();
    const game = await getAll(db.handle, 'game');
    if (!game.some(([k]) => /\/id1\/pak0\.pak$/i.test(k))) {
      status('id1/pak0.pak not found in the stored data.');
      return;
    }
    for (const [k, v] of game) vfs_mount(GAME_ROOT + lowerDir(k.slice(GAME_ROOT.length)), v);
    for (const [k, v] of await getAll(db.handle, 'user')) {
      if (!k.startsWith(USER_ROOT)) continue;
      // earlier versions stored the bytes only: give those a time that stays
      const kept = v instanceof Uint8Array ? { data: v } : v;
      vfs_mount(k, kept.data, kept.modified);
      if (kept.modified === undefined) changed(k);
    }
    const args = (new URLSearchParams(location.search).get('args') || '').split(' ').filter(Boolean);
    $('setup').hidden = true;
    canvas.hidden = false;
    canvas.focus();
    // ask not to be cleared out when space gets tight
    navigator.storage?.persist?.().catch(() => {});
    setInterval(() => persistDirty(db).catch(console.error), 2000);
    const leave = () => {
      try { page_hidden(); } catch (e) { console.error(e); }
      persistDirty(db, true).catch(console.error);
    };
    addEventListener('pagehide', leave);
    document.addEventListener('visibilitychange', () => { if (document.hidden) leave(); });
    await start(args);
  } catch (e) {
    $('setup').hidden = false;
    canvas.hidden = true;
    status(`Failed to start: ${e?.message || e}`);
    console.error(e);
  }
}

// Event handlers: an error is shown, not lost.
const shown = (f) => async (ev) => {
  try { await f(ev); } catch (e) { status(`Error: ${e?.message || e}`); console.error(e); }
};

async function main() {
  if (!navigator.gpu) {
    if ($('nogpu')) $('nogpu').hidden = false; // the page explains it itself
    else status('This browser has no WebGPU; the game cannot start.');
  }
  const db = { handle: await openDb() };
  const refresh = async () => {
    const have = (await getAll(db.handle, 'game')).length > 0;
    $('play').hidden = !have;
    $('forget').hidden = !have;
    if (have) status('Game data found in this browser. Press Play.');
  };
  $('pick').addEventListener('change', shown(async (ev) => {
    const list = [...ev.target.files].map((f) => [f.webkitRelativePath || f.name, f]);
    if (await storeFiles(db.handle, list)) await launch(db); else status('No usable files picked.');
  }));
  const drop = $('drop');
  drop.addEventListener('dragover', (e) => { e.preventDefault(); drop.classList.add('over'); });
  drop.addEventListener('dragleave', () => drop.classList.remove('over'));
  drop.addEventListener('drop', shown(async (e) => {
    e.preventDefault();
    drop.classList.remove('over');
    const entries = [...e.dataTransfer.items].map((i) => i.webkitGetAsEntry()).filter(Boolean);
    const list = [];
    for (const en of entries) await filesFromEntry(en, '', list);
    if (await storeFiles(db.handle, list)) await launch(db); else status('No usable files dropped.');
  }));
  $('play').addEventListener('click', () => launch(db));
  const sw = $('shareware');
  if (sw && sw.dataset.url) {
    sw.hidden = false;
    sw.addEventListener('click', async () => {
      sw.disabled = true;
      try {
        const have = (await getAll(db.handle, 'game')).some(([k]) => /\/id1\/pak0\.pak$/i.test(k));
        if (!have) await putAll(db.handle, 'game', [[`${GAME_ROOT}id1/pak0.pak`, await fetchShareware(sw.dataset.url)]]);
        await launch(db);
      } catch (e) {
        status(`Could not get the shareware data: ${e.message || e}`);
        console.error(e);
      } finally {
        sw.disabled = false;
      }
    });
  }
  $('forget').addEventListener('click', shown(async () => {
    await clearStore(db.handle, 'game');
    status('Stored game data removed (saves kept).');
    await refresh();
  }));
  await refresh();
}

main().catch((e) => { status(`Error: ${e}`); console.error(e); });
