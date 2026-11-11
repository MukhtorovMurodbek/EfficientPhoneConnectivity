package dev.epc

import android.app.Notification
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.json.JSONObject

/** Forwards notifications to the laptop. Does nothing unless connected and enabled there. */
class NotifListener : NotificationListenerService() {
    private val sent = HashMap<String, Int>() // key -> content hash, to skip no-op updates
    private var gen = -1
    private val labels = HashMap<String, String>()

    override fun onListenerConnected() {
        // Being bound by the system is a good moment to make sure the link is up.
        LinkService.start(this)
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        if (!Link.connected || !Link.notificationsEnabled) return
        if (gen != Link.generation) {
            sent.clear()
            gen = Link.generation
        }
        val n = sbn.notification
        if (sbn.packageName == packageName || sbn.isOngoing) return
        if (n.flags and (Notification.FLAG_GROUP_SUMMARY or Notification.FLAG_LOCAL_ONLY) != 0) return

        val ex = n.extras
        val title = ex.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (ex.getCharSequence(Notification.EXTRA_BIG_TEXT)
            ?: ex.getCharSequence(Notification.EXTRA_TEXT))?.toString().orEmpty()
        if (title.isEmpty() && text.isEmpty()) return

        val hash = (title + '\u0000' + text).hashCode()
        if (sent[sbn.key] == hash) return
        sent[sbn.key] = hash

        Link.send(
            JSONObject().put("t", "notif").put("key", sbn.key)
                .put("app", label(sbn.packageName)).put("title", title).put("text", text)
        )
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        if (sent.remove(sbn.key) != null && Link.connected) {
            Link.send(JSONObject().put("t", "notif_rm").put("key", sbn.key))
        }
    }

    private fun label(pkg: String) = labels.getOrPut(pkg) {
        runCatching {
            packageManager.getApplicationLabel(packageManager.getApplicationInfo(pkg, 0)).toString()
        }.getOrDefault(pkg)
    }
}
