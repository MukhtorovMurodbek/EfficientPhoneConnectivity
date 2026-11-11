package dev.epc

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothServerSocket
import android.bluetooth.BluetoothSocket
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.io.BufferedInputStream
import java.io.DataInputStream
import java.io.IOException

/**
 * Foreground service that sits in a blocking RFCOMM accept() — zero CPU while idle.
 * The laptop dials in; one connection at a time.
 */
class LinkService : Service() {
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var server: BluetoothServerSocket? = null
    @Volatile private var socket: BluetoothSocket? = null
    private var thread: Thread? = null
    private val onChange: () -> Unit = { main.post { notifyStatus() } }

    private val btState = object : BroadcastReceiver() {
        override fun onReceive(c: Context, i: Intent) {
            when (i.getIntExtra(BluetoothAdapter.EXTRA_STATE, -1)) {
                BluetoothAdapter.STATE_ON -> startListening()
                BluetoothAdapter.STATE_TURNING_OFF, BluetoothAdapter.STATE_OFF -> stopListening()
            }
        }
    }

    override fun onBind(intent: Intent?) = null

    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.channel_link), NotificationManager.IMPORTANCE_MIN)
        )
        startForeground(NOTIF_ID, buildNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        registerReceiver(btState, IntentFilter(BluetoothAdapter.ACTION_STATE_CHANGED))
        Link.addListener(onChange)
        running = true
        startListening()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int) = START_STICKY

    override fun onDestroy() {
        running = false
        Link.removeListener(onChange)
        unregisterReceiver(btState)
        stopListening()
        super.onDestroy()
    }

    @Synchronized
    private fun startListening() {
        if (thread?.isAlive == true) return
        val adapter = getSystemService(BluetoothManager::class.java).adapter ?: return
        if (!adapter.isEnabled) return
        thread = Thread({ acceptLoop(adapter) }, "epc-accept").apply { start() }
    }

    private fun stopListening() {
        runCatching { server?.close() }
        runCatching { socket?.close() }
    }

    private fun acceptLoop(adapter: BluetoothAdapter) {
        try {
            // Secure socket: only bonded devices, encrypted link. Registers the SDP record.
            val ss = adapter.listenUsingRfcommWithServiceRecord("EPC", Link.SERVICE_UUID)
            server = ss
            Log.i(Link.TAG, "listening")
            while (true) {
                val s = ss.accept()
                serve(s)
            }
        } catch (e: IOException) {
            Log.i(Link.TAG, "accept loop ended: $e")
        } catch (e: SecurityException) {
            Log.w(Link.TAG, "no bluetooth permission: $e")
        } finally {
            server = null
        }
    }

    private fun serve(s: BluetoothSocket) {
        socket = s
        val name = runCatching { s.remoteDevice.alias ?: s.remoteDevice.name }.getOrNull()
        Log.i(Link.TAG, "connected: $name")
        try {
            val input = DataInputStream(BufferedInputStream(s.inputStream))
            val out = s.outputStream
            Link.writeFrame(out, Link.hello(Build.MODEL))
            Link.attach(out, name)
            while (true) dispatch(Link.readFrame(input))
        } catch (e: Exception) {
            Log.i(Link.TAG, "disconnected: $e")
        } finally {
            Link.detach()
            runCatching { s.close() }
            socket = null
        }
    }

    private fun dispatch(m: org.json.JSONObject) {
        when (m.optString("t")) {
            "hello" -> {
                val v = m.optInt("v")
                if (v != Link.VERSION) Log.w(Link.TAG, "laptop speaks protocol v$v")
                m.optString("name").takeIf { it.isNotEmpty() }?.let { Link.peerName = it }
                Link.changed()
            }
            "cfg" -> {
                Link.clipboardEnabled = m.optBoolean("clipboard")
                Link.notificationsEnabled = m.optBoolean("notifications")
                Link.changed()
            }
            "clip" -> {
                val text = m.optString("text")
                if (Link.clipboardEnabled && text.isNotEmpty() && text != Link.lastClip) {
                    Link.lastClip = text
                    // Writing the clipboard is allowed from the background; reading is not.
                    main.post {
                        getSystemService(ClipboardManager::class.java)
                            .setPrimaryClip(ClipData.newPlainText(Link.peerName ?: "Laptop", text))
                    }
                }
            }
            // Unknown types are ignored (forward compatibility).
        }
    }

    private fun notifyStatus() {
        if (running) getSystemService(NotificationManager::class.java).notify(NOTIF_ID, buildNotification())
    }

    private fun buildNotification(): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE
        )
        val b = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat)
            .setOngoing(true)
            .setShowWhen(false)
            .setContentIntent(open)
            .setContentTitle(
                if (Link.connected) getString(R.string.connected_to, Link.peerName ?: "laptop")
                else getString(R.string.waiting)
            )
        if (Link.connected && Link.clipboardEnabled) {
            val send = PendingIntent.getActivity(
                this, 1, Intent(this, SendClipActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                PendingIntent.FLAG_IMMUTABLE
            )
            b.addAction(Notification.Action.Builder(null, getString(R.string.send_clipboard), send).build())
        }
        return b.build()
    }

    companion object {
        private const val CHANNEL = "link"
        private const val NOTIF_ID = 1
        @Volatile var running = false
            private set

        fun canRun(c: Context) =
            c.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED

        fun start(c: Context) {
            if (running || !canRun(c)) return
            try {
                c.startForegroundService(Intent(c, LinkService::class.java))
            } catch (e: Exception) {
                Log.w(Link.TAG, "could not start service: $e")
            }
        }
    }
}
