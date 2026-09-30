package org.terminus.mobile.terminal

import android.content.Context
import android.util.Log
import android.util.TypedValue
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.InputMethodManager
import com.termux.terminal.KeyHandler
import com.termux.view.TerminalView
import com.termux.view.TerminalViewClient
import com.termux.view.TerminalViewSession

/** Mesin terminal v1: Termux (mobile/DESIGN.md bagian 3 & 4.4). */
class TermuxEngine(private val appContext: Context) : TerminalEngine {
    override fun newSession(): EmulatorSession = TermuxSession(appContext)

    override fun newSurface(context: Context, modifiers: StickyModifiers, fontSize: FontSize): TerminalSurface =
        TermuxTerminalSurface(context, modifiers, fontSize)
}

/** [TerminalSurface] = `TerminalView` Termux (di-vendor, lihat THIRD_PARTY_NOTICES.md). */
private class TermuxTerminalSurface(
    context: Context,
    private val modifiers: StickyModifiers,
    private val fontSize: FontSize,
) : TerminalSurface {
    private val terminalView = TerminalView(context, null)
    private var session: TermuxSession? = null

    override val view: View get() = terminalView

    init {
        terminalView.setTerminalViewClient(Client())
        terminalView.isFocusable = true
        terminalView.isFocusableInTouchMode = true
        applyFontSize()
    }

    override fun show(session: EmulatorSession?) {
        val next = session as TermuxSession?
        if (next === this.session) return
        this.session?.onScreenUpdated = null
        this.session = next
        if (next != null) {
            next.onScreenUpdated = { terminalView.onScreenUpdated() }
            terminalView.attachSession(next)
            terminalView.onScreenUpdated()
        }
    }

    override fun press(key: SpecialKey) {
        val s = session ?: return
        if (s.getEmulator() == null) return // layar belum diukur
        val keyCode = when (key) {
            SpecialKey.Escape -> KeyEvent.KEYCODE_ESCAPE
            SpecialKey.Tab -> KeyEvent.KEYCODE_TAB
            SpecialKey.Left -> KeyEvent.KEYCODE_DPAD_LEFT
            SpecialKey.Up -> KeyEvent.KEYCODE_DPAD_UP
            SpecialKey.Down -> KeyEvent.KEYCODE_DPAD_DOWN
            SpecialKey.Right -> KeyEvent.KEYCODE_DPAD_RIGHT
        }
        var mod = 0
        if (modifiers.consumeCtrl()) mod = mod or KeyHandler.KEYMOD_CTRL
        if (modifiers.consumeAlt()) mod = mod or KeyHandler.KEYMOD_ALT
        terminalView.handleKeyCode(keyCode, mod)
    }

    // inputCodePoint membaca Ctrl/Alt tempel sendiri lewat Client.readControlKey().
    override fun type(char: Char) = terminalView.inputCodePoint(char.code, false, false)

    override fun pasteClipboard() {
        session?.onPasteTextFromClipboard()
    }

    override fun showKeyboard() {
        terminalView.requestFocus()
        terminalView.context.getSystemService(InputMethodManager::class.java)
            .showSoftInput(terminalView, 0)
    }

    private fun applyFontSize() {
        val px = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, fontSize.sp, terminalView.resources.displayMetrics)
        terminalView.setTextSize(px.toInt())
    }

    private inner class Client : TerminalViewClient {
        /** Pinch: sama dengan Termux — ubah font per langkah, kembalikan 1 = skala di-reset. */
        override fun onScale(scale: Float): Float {
            if (scale in 0.9f..1.1f) return scale
            fontSize.set(fontSize.sp + if (scale > 1f) 1f else -1f)
            applyFontSize()
            return 1f
        }

        override fun onSingleTapUp(e: MotionEvent) = showKeyboard()

        override fun shouldBackButtonBeMappedToEscape() = false
        override fun shouldEnforceCharBasedInput() = false
        override fun shouldUseCtrlSpaceWorkaround() = false
        override fun isTerminalViewSelected() = true
        override fun copyModeChanged(copyMode: Boolean) = Unit
        override fun onKeyDown(keyCode: Int, e: KeyEvent, session: TerminalViewSession) = false
        override fun onKeyUp(keyCode: Int, e: KeyEvent) = false
        override fun onLongPress(event: MotionEvent) = false // biarkan view memulai seleksi teks
        override fun readControlKey() = modifiers.consumeCtrl()
        override fun readAltKey() = modifiers.consumeAlt()
        override fun readShiftKey() = false
        override fun readFnKey() = false
        override fun onCodePoint(codePoint: Int, ctrlDown: Boolean, session: TerminalViewSession) = false
        override fun onEmulatorSet() = Unit
        override fun logError(tag: String?, message: String?) { Log.e(TAG, "$tag: $message") }
        override fun logWarn(tag: String?, message: String?) { Log.w(TAG, "$tag: $message") }
        override fun logInfo(tag: String?, message: String?) = Unit
        override fun logDebug(tag: String?, message: String?) = Unit
        override fun logVerbose(tag: String?, message: String?) = Unit
        override fun logStackTraceWithMessage(tag: String?, message: String?, e: Exception?) { Log.e(TAG, "$tag: $message", e) }
        override fun logStackTrace(tag: String?, e: Exception?) { Log.e(TAG, tag, e) }
    }

    private companion object {
        const val TAG = "TerminalSurface"
    }
}
