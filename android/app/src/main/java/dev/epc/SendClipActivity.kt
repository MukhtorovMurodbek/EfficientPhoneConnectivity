package dev.epc

import android.app.Activity
import android.content.ClipboardManager
import android.content.Intent
import android.os.Bundle
import android.widget.Toast

/**
 * Invisible activity. Android only lets the focused app read the clipboard, so we
 * wait for window focus, read, send, and close. Also handles "Share → Send to laptop".
 */
class SendClipActivity : Activity() {
    private var done = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (intent?.action == Intent.ACTION_SEND) {
            finishWith(Link.sendClip(intent.getStringExtra(Intent.EXTRA_TEXT)))
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || done) return
        val clip = getSystemService(ClipboardManager::class.java).primaryClip
        val text = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString()
        finishWith(Link.sendClip(text))
    }

    private fun finishWith(r: Link.ClipResult) {
        done = true
        val msg = when (r) {
            Link.ClipResult.SENT -> R.string.sent
            Link.ClipResult.NOT_CONNECTED -> R.string.not_connected
            Link.ClipResult.DISABLED -> R.string.clipboard_off
            Link.ClipResult.EMPTY -> R.string.clipboard_empty
        }
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()
        finish()
    }
}
