package org.terminus.mobile.terminal

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import com.termux.terminal.TerminalEmulator
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient
import com.termux.view.TerminalViewSession
import java.io.ByteArrayOutputStream
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import org.terminus.mobile.ssh.ShellChannel

/**
 * [EmulatorSession] versi Termux: `TerminalEmulator` (layar + riwayat)
 * yang diberi makan byte dari [ShellChannel].
 *
 * Aturan thread (sama dengan `TerminalSession` Termux): emulator HANYA
 * disentuh di main thread; baca jaringan di thread sendiri, tulis/tutup
 * jaringan di satu executor (urutan input terjaga, main thread tidak
 * pernah blok).
 */
class TermuxSession(context: Context) : TerminalViewSession(), EmulatorSession, TerminalSessionClient {
    private val clipboard = context.applicationContext.getSystemService(ClipboardManager::class.java)
    private val main = Handler(Looper.getMainLooper())
    private val writer: ExecutorService = Executors.newSingleThreadExecutor { Thread(it, "terminus-ssh-writer") }

    private var emulator: TerminalEmulator? = null
    /** Output yang datang sebelum view pertama kali mengukur layar (emulator belum ada). */
    private val pending = ByteArrayOutputStream()
    private var channel: ShellChannel? = null
    /** Naik tiap attach/detach — pembaca lama yang baru selesai tidak boleh melapor "terputus". */
    private var generation = 0
    private var disposed = false

    override var columns = DEFAULT_COLUMNS
        private set
    override var rows = DEFAULT_ROWS
        private set

    override var onChannelEnded: ((Throwable?) -> Unit)? = null

    /** Diisi view yang sedang menampilkan sesi ini. */
    var onScreenUpdated: (() -> Unit)? = null

    override fun getEmulator(): TerminalEmulator? = emulator

    override fun updateSize(columns: Int, rows: Int, cellWidthPixels: Int, cellHeightPixels: Int) {
        this.columns = columns
        this.rows = rows
        val current = emulator
        if (current == null) {
            emulator = TerminalEmulator(this, columns, rows, cellWidthPixels, cellHeightPixels, TRANSCRIPT_ROWS, this)
            if (pending.size() > 0) {
                appendToScreen(pending.toByteArray())
                pending.reset()
            }
        } else {
            current.resize(columns, rows, cellWidthPixels, cellHeightPixels)
        }
        channel?.let { ch -> execute { ch.resize(columns, rows) } }
    }

    override fun attach(channel: ShellChannel) {
        detach()
        this.channel = channel
        val myGeneration = ++generation
        // PTY diminta dengan ukuran saat connect DIMULAI; layar bisa sudah
        // berubah (rotasi, keyboard) selama menunggu.
        execute { channel.resize(columns, rows) }
        Thread({
            var error: Throwable? = null
            try {
                val buffer = ByteArray(BUFFER_SIZE)
                while (true) {
                    val read = channel.output.read(buffer)
                    if (read < 0) break
                    val chunk = buffer.copyOf(read)
                    main.post { if (generation == myGeneration) appendToScreen(chunk) }
                }
            } catch (e: Throwable) {
                error = e
            }
            main.post {
                if (generation == myGeneration && !disposed) {
                    this.channel = null
                    execute { channel.close() }
                    onChannelEnded?.invoke(error)
                }
            }
        }, "terminus-ssh-reader").start()
    }

    override fun detach() {
        generation++
        val old = channel ?: return
        channel = null
        execute { old.close() }
    }

    override fun printNotice(text: String) {
        appendToScreen("\r\n\u001b[33m[$text]\u001b[0m\r\n".toByteArray())
    }

    override fun dispose() {
        detach()
        disposed = true
        onScreenUpdated = null
        onChannelEnded = null
        writer.shutdown() // tugas yang sudah antre (mis. close) tetap dijalankan
    }

    private fun appendToScreen(bytes: ByteArray) {
        val e = emulator
        if (e == null) {
            pending.write(bytes)
            return
        }
        e.append(bytes, bytes.size)
        onScreenUpdated?.invoke()
    }

    private fun execute(block: () -> Unit) {
        if (writer.isShutdown) return
        writer.execute { runCatching(block) }
    }

    // --- TerminalOutput: arah emulator -> server / sistem ---

    override fun write(data: ByteArray, offset: Int, count: Int) {
        val ch = channel ?: return
        val copy = data.copyOfRange(offset, offset + count)
        execute { ch.write(copy) }
    }

    override fun titleChanged(oldTitle: String?, newTitle: String?) = Unit

    override fun onCopyTextToClipboard(text: String?) {
        if (!text.isNullOrEmpty()) clipboard.setPrimaryClip(ClipData.newPlainText("Terminal", text))
    }

    override fun onPasteTextFromClipboard() {
        val text = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text?.toString()
        if (!text.isNullOrEmpty()) emulator?.paste(text)
    }

    override fun onBell() = Unit

    override fun onColorsChanged() {
        onScreenUpdated?.invoke()
    }

    // --- TerminalSessionClient: cuma dipakai emulator untuk log & gaya kursor.
    // Callback bertipe TerminalSession (proses lokal) tidak pernah dipanggil di sini.

    override fun onTextChanged(changedSession: TerminalSession) = Unit
    override fun onTitleChanged(changedSession: TerminalSession) = Unit
    override fun onSessionFinished(finishedSession: TerminalSession) = Unit
    override fun onCopyTextToClipboard(session: TerminalSession, text: String?) = onCopyTextToClipboard(text)
    override fun onPasteTextFromClipboard(session: TerminalSession?) = onPasteTextFromClipboard()
    override fun onBell(session: TerminalSession) = Unit
    override fun onColorsChanged(session: TerminalSession) = Unit
    override fun onTerminalCursorStateChange(state: Boolean) = Unit
    override fun getTerminalCursorStyle(): Int = TerminalEmulator.DEFAULT_TERMINAL_CURSOR_STYLE
    override fun logError(tag: String?, message: String?) { Log.e(TAG, "$tag: $message") }
    override fun logWarn(tag: String?, message: String?) { Log.w(TAG, "$tag: $message") }
    override fun logInfo(tag: String?, message: String?) = Unit
    override fun logDebug(tag: String?, message: String?) = Unit
    override fun logVerbose(tag: String?, message: String?) = Unit
    override fun logStackTraceWithMessage(tag: String?, message: String?, e: Exception?) { Log.e(TAG, "$tag: $message", e) }
    override fun logStackTrace(tag: String?, e: Exception?) { Log.e(TAG, tag, e) }

    companion object {
        private const val TAG = "TermuxSession"
        const val DEFAULT_COLUMNS = 80
        const val DEFAULT_ROWS = 24
        private const val TRANSCRIPT_ROWS = 5000
        private const val BUFFER_SIZE = 8192
    }
}
