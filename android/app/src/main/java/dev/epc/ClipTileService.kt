package dev.epc

import android.app.PendingIntent
import android.content.Intent
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/** Quick Settings tile: "Send clipboard" to the laptop. */
class ClipTileService : TileService() {
    override fun onStartListening() {
        qsTile?.apply {
            state = if (Link.connected && Link.clipboardEnabled) Tile.STATE_INACTIVE else Tile.STATE_UNAVAILABLE
            updateTile()
        }
    }

    override fun onClick() {
        val i = Intent(this, SendClipActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        startActivityAndCollapse(PendingIntent.getActivity(this, 0, i, PendingIntent.FLAG_IMMUTABLE))
    }
}
