package org.quakesourceport.app;

import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;
import android.util.AtomicFile;
import android.util.Log;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.RandomAccessFile;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;
import java.util.regex.Pattern;

final class GameData {
    private static final String FIRST = "library-a";
    private static final String SECOND = "library-b";
    private static final String TAG = "QuakeGameData";
    private static final Pattern PAK_NAME = Pattern.compile("pak(0|[1-9][0-9]*)\\.pak");

    private final ContentResolver resolver;
    private final File external;
    private final AtomicFile selection;

    GameData(Context context) {
        resolver = context.getContentResolver();
        external = context.getExternalFilesDir(null);
        selection = new AtomicFile(new File(context.getFilesDir(), "game-data-slot"));
    }

    File importedBase() {
        if (external == null) return null;
        try {
            String slot = selectedSlot();
            if (slot == null) return null;
            File base = new File(library(), slot);
            return new File(base, "id1/pak0.pak").isFile() ? base : null;
        } catch (IOException e) {
            Log.w(TAG, "Could not read the selected PAK import", e);
            return null;
        }
    }

    File legacyBase() {
        if (external == null) return null;
        File id1 = new File(external, "id1");
        if (id1.isDirectory() && new File(id1, "gfx/palette.lmp").isFile()) return external;
        File[] files = (id1.isDirectory() ? id1 : external).listFiles();
        if (files != null) {
            for (File file : files) {
                if (file.isFile() && file.getName().equalsIgnoreCase("pak0.pak")) return external;
            }
        }
        return null;
    }

    void importPaks(Uri tree) throws IOException {
        if (external == null) throw new IOException("App storage is unavailable.");

        String root = DocumentsContract.getTreeDocumentId(tree);
        Map<String, Uri> paks = new TreeMap<>();
        String id1 = findPaks(tree, root, paks);
        if (id1 != null) {
            Map<String, Uri> nestedPaks = new TreeMap<>();
            findPaks(tree, id1, nestedPaks);
            if (nestedPaks.containsKey("pak0.pak")) paks = nestedPaks;
        }
        if (!paks.containsKey("pak0.pak")) {
            throw new IOException("Choose the folder containing pak0.pak or its parent Quake folder.");
        }

        // Copy to the inactive slot. A failed or interrupted copy leaves the active slot intact.
        String previous = selectedSlot();
        String next = FIRST.equals(previous) ? SECOND : FIRST;
        File target = new File(library(), next);
        File targetId1 = new File(target, "id1");
        removeSlot(target);
        try {
            if (!targetId1.mkdirs()) throw new IOException("Could not create the import folder.");
            byte[] buffer = new byte[64 * 1024];
            for (Map.Entry<String, Uri> pak : paks.entrySet()) {
                File output = new File(targetId1, pak.getKey());
                copyPak(pak.getValue(), output, buffer);
                if (pak.getKey().equals("pak0.pak")) validateBasePak(output);
            }
            select(next);
        } catch (IOException | RuntimeException e) {
            try {
                removeSlot(target);
            } catch (IOException cleanup) {
                e.addSuppressed(cleanup);
            }
            throw e;
        }
        if (previous != null) {
            try {
                removeSlot(new File(library(), previous));
            } catch (IOException e) {
                Log.w(TAG, "Could not remove the previous PAK import", e);
            }
        }
    }

    private File library() {
        return new File(external, "game-data");
    }

    private String selectedSlot() throws IOException {
        try {
            String slot = new String(selection.readFully(), StandardCharsets.UTF_8);
            return FIRST.equals(slot) || SECOND.equals(slot) ? slot : null;
        } catch (FileNotFoundException e) {
            return null;
        }
    }

    private void select(String slot) throws IOException {
        FileOutputStream output = selection.startWrite();
        try {
            output.write(slot.getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            selection.failWrite(output);
            throw e;
        }
        selection.finishWrite(output);
        if (!slot.equals(selectedSlot())) throw new IOException("Could not save the selected PAK import.");
    }

    private String findPaks(Uri tree, String directory, Map<String, Uri> paks) throws IOException {
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, directory);
        String id1 = null;
        try (Cursor cursor = resolver.query(children, new String[] {
                Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE
        }, null, null, null)) {
            if (cursor == null) throw new IOException("Could not list the selected folder.");
            while (cursor.moveToNext()) {
                String name = cursor.getString(1).toLowerCase(Locale.ROOT);
                String id = cursor.getString(0);
                if (Document.MIME_TYPE_DIR.equals(cursor.getString(2))) {
                    if (name.equals("id1")) id1 = id;
                } else if (PAK_NAME.matcher(name).matches()) {
                    Uri uri = DocumentsContract.buildDocumentUriUsingTree(tree, id);
                    if (paks.putIfAbsent(name, uri) != null) {
                        throw new IOException("The selected folder has multiple copies of " + name + ".");
                    }
                }
            }
        }
        return id1;
    }

    private void copyPak(Uri uri, File output, byte[] buffer) throws IOException {
        try (InputStream input = resolver.openInputStream(uri)) {
            if (input == null) throw new IOException("Could not open " + output.getName() + ".");
            try (FileOutputStream file = new FileOutputStream(output)) {
                for (int count; (count = input.read(buffer)) != -1;) file.write(buffer, 0, count);
                file.getFD().sync();
            }
        }
    }

    static void validateBasePak(File pak) throws IOException {
        // Check the PAK directory for the palette the engine needs at startup.
        try (RandomAccessFile file = new RandomAccessFile(pak, "r")) {
            long length = file.length();
            if (length >= 12 && file.readInt() == 0x5041434b) { // PACK
                int offset = Integer.reverseBytes(file.readInt());
                int size = Integer.reverseBytes(file.readInt());
                if (offset >= 12 && size > 0 && size % 64 == 0 && (long) offset + size <= length) {
                    byte[] name = new byte[56];
                    file.seek(offset);
                    for (int entry = 0; entry < size / 64; entry++) {
                        file.readFully(name);
                        int start = Integer.reverseBytes(file.readInt());
                        int bytes = Integer.reverseBytes(file.readInt());
                        int end = 0;
                        while (end < name.length && name[end] != 0) end++;
                        if (new String(name, 0, end, StandardCharsets.US_ASCII)
                                .equalsIgnoreCase("gfx/palette.lmp")
                                && start >= 12 && bytes >= 768 && (long) start + bytes <= length) return;
                    }
                }
            }
        }
        throw new IOException("pak0.pak is missing the Quake palette or is damaged.");
    }

    private static void removeSlot(File path) throws IOException {
        File[] children = path.listFiles();
        if (children != null) {
            for (File child : children) removeSlot(child);
        }
        if (path.exists() && !path.delete()) throw new IOException("Could not clear an old PAK import.");
    }
}
