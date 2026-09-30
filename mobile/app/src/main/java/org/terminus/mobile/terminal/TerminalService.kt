package org.terminus.mobile.terminal

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.terminus.mobile.MainActivity
import org.terminus.mobile.R
import org.terminus.mobile.TerminusApplication

/**
 * Foreground service "N sesi SSH aktif" (mobile/DESIGN.md bagian 7). TIDAK
 * memegang koneksi — koneksi ada di [TerminalSessions] (seumur proses);
 * service ini cuma memberi tahu Android bahwa proses sedang dipakai,
 * supaya sesi tidak dimatikan waktu app di background. Berhenti sendiri
 * begitu sesi terakhir ditutup.
 */
class TerminalService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var observing = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val sessions = (application as TerminusApplication).container.terminalSessions
        // startForeground WAJIB segera setelah startForegroundService (batas waktu Android).
        startForeground(NOTIFICATION_ID, notification(sessions.tabs.value.size), ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        if (!observing) {
            observing = true
            scope.launch {
                sessions.tabs.collect { tabs ->
                    if (tabs.isEmpty()) {
                        stopForeground(STOP_FOREGROUND_REMOVE)
                        stopSelf()
                    } else {
                        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification(tabs.size))
                    }
                }
            }
        }
        // Proses mati (mis. dibunuh sistem) = koneksi ikut hilang; jangan hidupkan ulang service kosong.
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun notification(count: Int): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_server)
            .setContentTitle("$count sesi SSH aktif")
            .setContentText("Ketuk untuk kembali ke Terminus.")
            .setContentIntent(open)
            .setOngoing(true)
            .build()
    }

    companion object {
        private const val CHANNEL_ID = "ssh_sessions"
        private const val NOTIFICATION_ID = 1

        /** Dipanggil tiap sesi dibuka (dari foreground — diizinkan Android). */
        fun start(context: Context) {
            val manager = context.getSystemService(NotificationManager::class.java)
            if (manager.getNotificationChannel(CHANNEL_ID) == null) {
                manager.createNotificationChannel(
                    NotificationChannel(CHANNEL_ID, "Sesi SSH aktif", NotificationManager.IMPORTANCE_LOW),
                )
            }
            context.startForegroundService(Intent(context, TerminalService::class.java))
        }
    }
}
