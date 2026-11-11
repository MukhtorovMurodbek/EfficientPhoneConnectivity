package dev.epc

import android.util.Log
import org.json.JSONObject
import java.io.DataInputStream
import java.io.IOException
import java.io.OutputStream
import java.nio.ByteBuffer
import java.util.UUID
import java.util.concurrent.Executors

/** Process-wide link state + wire protocol v1 (see PROTOCOL.md). */
object Link {
    const val TAG = "epc"
    val SERVICE_UUID: UUID = UUID.fromString("540c3dea-5e95-4872-993a-c1e25e3b021d")
    const val VERSION = 1
    private const val MAX_FRAME = 1 shl 20

    // Sync switches are owned by the laptop (tray menu) and pushed via `cfg`.
    @Volatile var clipboardEnabled = false
    @Volatile var notificationsEnabled = false
    @Volatile var peerName: String? = null
    /** Last clipboard text sent or received — echo suppression. */
    @Volatile var lastClip: String? = null
    /** Bumped on every new connection so listeners can reset per-connection state. */
    @Volatile var generation = 0
        private set

    @Volatile private var out: OutputStream? = null
    private val writer = Executors.newSingleThreadExecutor()
    private val listeners = mutableSetOf<() -> Unit>()

    val connected get() = out != null

    fun attach(o: OutputStream, name: String?) {
        out = o
        peerName = name
        generation++
        changed()
    }

    fun detach() {
        out = null
        clipboardEnabled = false
        notificationsEnabled = false
        changed()
    }

    fun addListener(l: () -> Unit) = synchronized(listeners) { listeners += l }
    fun removeListener(l: () -> Unit) = synchronized(listeners) { listeners -= l }
    fun changed() = synchronized(listeners) { listeners.toList() }.forEach { it() }

    /** Queue a message; never blocks the caller. */
    fun send(msg: JSONObject) {
        val o = out ?: return
        writer.execute {
            try {
                writeFrame(o, msg)
            } catch (e: IOException) {
                Log.w(TAG, "send failed: $e") // reader thread sees the broken socket and detaches
            }
        }
    }

    enum class ClipResult { SENT, NOT_CONNECTED, DISABLED, EMPTY }

    fun sendClip(text: String?): ClipResult = when {
        text.isNullOrEmpty() -> ClipResult.EMPTY
        !connected -> ClipResult.NOT_CONNECTED
        !clipboardEnabled -> ClipResult.DISABLED
        else -> {
            lastClip = text
            send(JSONObject().put("t", "clip").put("text", text))
            ClipResult.SENT
        }
    }

    fun hello(name: String) = JSONObject().put("t", "hello").put("v", VERSION).put("name", name)

    fun writeFrame(o: OutputStream, msg: JSONObject) {
        val body = msg.toString().toByteArray(Charsets.UTF_8)
        if (body.size > MAX_FRAME) throw IOException("message too large")
        // Header + body in one write so RFCOMM sends one packet.
        val buf = ByteBuffer.allocate(4 + body.size).putInt(body.size).put(body).array()
        synchronized(o) {
            o.write(buf)
            o.flush()
        }
    }

    fun readFrame(i: DataInputStream): JSONObject {
        val n = i.readInt()
        if (n < 1 || n > MAX_FRAME) throw IOException("bad frame length $n")
        val b = ByteArray(n)
        i.readFully(b)
        return JSONObject(String(b, Charsets.UTF_8))
    }
}
