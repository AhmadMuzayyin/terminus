// BUKAN kode Termux — ditulis untuk Terminus. Pengganti `TerminalSession`
// Termux (final & selalu menjalankan proses lokal lewat JNI) di view yang
// di-vendor dari terminal-view v0.118.3: cuma bagian yang benar-benar
// dipakai `TerminalView`. Implementasi: `org.terminus.mobile.terminal.TermuxSession`.
package com.termux.view;

import com.termux.terminal.TerminalEmulator;
import com.termux.terminal.TerminalOutput;

public abstract class TerminalViewSession extends TerminalOutput {

    private final byte[] mUtf8InputBuffer = new byte[5];

    /** Null sampai ukuran layar pertama diketahui ({@link #updateSize}). */
    public abstract TerminalEmulator getEmulator();

    /** Ukuran view berubah (juga panggilan pertama: buat emulator). Dipanggil di main thread. */
    public abstract void updateSize(int columns, int rows, int cellWidthPixels, int cellHeightPixels);

    /** Tulis satu code point (UTF-8), opsional diawali ESC (Alt). Logika sama dengan TerminalSession Termux. */
    public void writeCodePoint(boolean prependEscape, int codePoint) {
        if (codePoint > 1114111 || (codePoint >= 0xD800 && codePoint <= 0xDFFF)) {
            throw new IllegalArgumentException("Invalid code point: " + codePoint);
        }
        int pos = 0;
        if (prependEscape) mUtf8InputBuffer[pos++] = 27;
        if (codePoint <= 0x7F) {
            mUtf8InputBuffer[pos++] = (byte) codePoint;
        } else if (codePoint <= 0x7FF) {
            mUtf8InputBuffer[pos++] = (byte) (0b11000000 | (codePoint >> 6));
            mUtf8InputBuffer[pos++] = (byte) (0b10000000 | (codePoint & 0b111111));
        } else if (codePoint <= 0xFFFF) {
            mUtf8InputBuffer[pos++] = (byte) (0b11100000 | (codePoint >> 12));
            mUtf8InputBuffer[pos++] = (byte) (0b10000000 | ((codePoint >> 6) & 0b111111));
            mUtf8InputBuffer[pos++] = (byte) (0b10000000 | (codePoint & 0b111111));
        } else {
            mUtf8InputBuffer[pos++] = (byte) (0b11110000 | (codePoint >> 18));
            mUtf8InputBuffer[pos++] = (byte) (0b10000000 | ((codePoint >> 12) & 0b111111));
            mUtf8InputBuffer[pos++] = (byte) (0b10000000 | ((codePoint >> 6) & 0b111111));
            mUtf8InputBuffer[pos++] = (byte) (0b10000000 | (codePoint & 0b111111));
        }
        write(mUtf8InputBuffer, 0, pos);
    }
}
