# qbj3 entry-door label investigation

Investigated from master `cad3dd8` on branch `fix/qbj3-level-labels`.

The labels use ordinary Quake centerprints containing 8-bit conchars indices.
Their colored letters and difficulty icons were corrupted by UTF-8 decoding
before the renderer received them.

## Evidence

The first `Avg Playtime` message in the installed qbj3 `maps/start.bsp`
starts with these title bytes:

```text
a0 d0 e5 ee e4 f2 e1 e7 ef ee a0
```

These are 11 font indices: ` Pendragon ` with bit 7 set to select the
alternate font color. The rest of the message includes `b5 8a` for the
playtime and `9e 9d 9d` for difficulty icons. These are not UTF-8.

Previously, `qformats::Bsp::parse` decoded the entity lump using
`qformats::cstr`, which calls `String::from_utf8_lossy`. The 11 title bytes became ten
replacement characters, encoded as 30 bytes (`ef bf bd` repeated).
The entity parser subsequently passes those UTF-8 bytes into the VM.
The original font indices are irrecoverably lost at BSP load time.

There was a second independent corruption point: `SVC_CENTERPRINT` in
`crates/qclient/src/parse.rs` used `MsgReader::string_lossy`, which would
also corrupt a correctly encoded centerprint from an external server or QC.

`Console::draw_center_scaled` measured strings with `len()` and drew
`bytes()` as font indices. Consequently it drew the replacement character's
three UTF-8 bytes as three unrelated glyphs and centers the inflated width.
The renderer is behaving consistently with its byte-oriented font interface.

The source `.map` spells the color switches as `\b`, but the compiled BSP
already contains high-bit characters and no literal `\b` sequences.
Adding support for that escape in `Strings::new_string` would not fix this
map. Its existing escape behavior also matches the local Ironwail reference.

## Fix

`qcore::text` now explicitly carries font bytes as same-valued Unicode scalars
through existing string APIs. Entity loading, including external `.ent` files,
encodes those scalars back into the original bytes for the VM. Print,
centerprint and finale messages decode losslessly; centerprint wrapping and
layout count actual glyphs. UI font/localization files remain UTF-8.

Save serialization and manual/autosave disk I/O preserve the original bytes;
save previews read the same format. This prevents labels from becoming corrupt
again after restoring a save. Previously corrupted saves cannot recover the
discarded bytes; start a new level to obtain intact labels.

## Validation

Synthetic regressions exercise BSP entity decoding, all 256 font indices,
network print/centerprint/finale messages, wrapping, centering and typewriter
limits. Game-data tests cover entity parsing and v5 save/load, plus the actual
qbj3 start-map label through VM allocation, client decoding and save restore;
they skip when the required game data is absent.

A release headless capture of qbj3 start at player origin `464 -992 2080`
shows the centered orange Pendragon title and Quiet credit, 5-minute playtime,
and three difficulty icons correctly (`/tmp/qbj3-label-fixed.png`).
