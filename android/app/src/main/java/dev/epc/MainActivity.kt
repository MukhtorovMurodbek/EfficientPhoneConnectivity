package dev.epc

import android.Manifest
import android.app.Activity
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.view.WindowInsets
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast

/** Setup + status screen. Sync switches live in the laptop's tray menu. */
class MainActivity : Activity() {
    private lateinit var status: TextView
    private lateinit var perms: Button
    private lateinit var notifAccess: Button
    private lateinit var battery: Button
    private val onChange: () -> Unit = { runOnUiThread { refresh() } }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pad = (20 * resources.displayMetrics.density).toInt()
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(pad, pad, pad, pad)
        }
        fun button(label: Int, onClick: () -> Unit) = Button(this).apply {
            setText(label)
            setOnClickListener { onClick() }
            col.addView(this)
        }

        col.addView(TextView(this).apply {
            setText(R.string.app_name)
            textSize = 26f
        })
        status = TextView(this).apply {
            textSize = 16f
            setPadding(0, pad / 2, 0, pad)
        }
        col.addView(status)

        perms = button(R.string.grant_permissions) {
            requestPermissions(
                arrayOf(Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.POST_NOTIFICATIONS), 1
            )
        }
        notifAccess = button(R.string.grant_notification_access) {
            startActivity(
                Intent(Settings.ACTION_NOTIFICATION_LISTENER_DETAIL_SETTINGS).putExtra(
                    Settings.EXTRA_NOTIFICATION_LISTENER_COMPONENT_NAME,
                    ComponentName(this, NotifListener::class.java).flattenToString()
                )
            )
        }
        battery = button(R.string.allow_background) {
            startActivity(
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName"))
            )
        }
        button(R.string.bluetooth_settings) { startActivity(Intent(Settings.ACTION_BLUETOOTH_SETTINGS)) }
        button(R.string.send_clipboard) {
            // We're in the foreground here, so reading the clipboard is allowed.
            val clip = getSystemService(ClipboardManager::class.java).primaryClip
            val text = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString()
            val msg = when (Link.sendClip(text)) {
                Link.ClipResult.SENT -> R.string.sent
                Link.ClipResult.NOT_CONNECTED -> R.string.not_connected
                Link.ClipResult.DISABLED -> R.string.clipboard_off
                Link.ClipResult.EMPTY -> R.string.clipboard_empty
            }
            Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()
        }
        col.addView(TextView(this).apply {
            setText(R.string.help)
            setPadding(0, pad, 0, 0)
            gravity = Gravity.START
        })

        val root = ScrollView(this).apply { addView(col) }
        // Edge-to-edge is enforced on new targets: keep content clear of system bars.
        root.setOnApplyWindowInsetsListener { v, insets ->
            val b = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            v.setPadding(b.left, b.top, b.right, b.bottom)
            WindowInsets.CONSUMED
        }
        setContentView(root)
    }

    override fun onResume() {
        super.onResume()
        Link.addListener(onChange)
        LinkService.start(this)
        refresh()
    }

    override fun onPause() {
        Link.removeListener(onChange)
        super.onPause()
    }

    override fun onRequestPermissionsResult(code: Int, p: Array<out String>, r: IntArray) {
        LinkService.start(this)
        refresh()
    }

    private fun refresh() {
        val btGranted = checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED
        val listenerOn = Settings.Secure.getString(contentResolver, "enabled_notification_listeners")
            ?.contains(ComponentName(this, NotifListener::class.java).flattenToString()) == true
        val unrestricted = getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(packageName)

        perms.visibility = if (btGranted) View.GONE else View.VISIBLE
        notifAccess.visibility = if (listenerOn) View.GONE else View.VISIBLE
        battery.visibility = if (unrestricted) View.GONE else View.VISIBLE

        status.text = buildString {
            append(
                when {
                    !btGranted -> getString(R.string.needs_permission)
                    Link.connected -> getString(R.string.connected_to, Link.peerName ?: "laptop")
                    else -> getString(R.string.waiting)
                }
            )
            if (Link.connected) {
                append("\n")
                append(getString(R.string.sync_state,
                    onOff(Link.clipboardEnabled), onOff(Link.notificationsEnabled && listenerOn)))
            }
        }
    }

    private fun onOff(b: Boolean) = getString(if (b) R.string.on else R.string.off)
}
