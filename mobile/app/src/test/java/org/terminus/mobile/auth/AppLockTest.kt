package org.terminus.mobile.auth

import org.junit.Assert.assertEquals
import org.junit.Test

/** Aturan kunci app, mobile/DESIGN.md bagian 6. */
class AppLockTest {
    private var now = 0L
    private val lock = AppLock(clock = { now }, timeoutMillis = 5 * 60_000L)

    @Test fun `belum baca pengaturan - Unknown, kunci mati - langsung terbuka`() {
        assertEquals(LockState.Unknown, lock.state.value)
        lock.onSettings(enabled = false)
        assertEquals(LockState.Unlocked, lock.state.value)
    }

    @Test fun `kunci aktif - proses baru langsung terkunci sampai unlock`() {
        lock.onSettings(enabled = true)
        assertEquals(LockState.Locked, lock.state.value)
        lock.unlock()
        assertEquals(LockState.Unlocked, lock.state.value)
    }

    @Test fun `background kurang dari 5 menit tetap terbuka, 5 menit atau lebih terkunci`() {
        lock.onSettings(enabled = true)
        lock.unlock()

        lock.onBackground(); now += 4 * 60_000L; lock.onForeground()
        assertEquals(LockState.Unlocked, lock.state.value)

        lock.onBackground(); now += 5 * 60_000L; lock.onForeground()
        assertEquals(LockState.Locked, lock.state.value)
    }

    @Test fun `kunci mati - background lama tidak mengunci, dimatikan saat terkunci - terbuka`() {
        lock.onSettings(enabled = false)
        lock.onBackground(); now += 60 * 60_000L; lock.onForeground()
        assertEquals(LockState.Unlocked, lock.state.value)

        lock.onSettings(enabled = true) // baru diaktifkan di Pengaturan: tidak langsung mengunci
        assertEquals(LockState.Unlocked, lock.state.value)
        lock.onBackground(); now += 10 * 60_000L; lock.onForeground()
        assertEquals(LockState.Locked, lock.state.value)
        lock.onSettings(enabled = false)
        assertEquals(LockState.Unlocked, lock.state.value)
    }
}
