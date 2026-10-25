package uz.harfiy.keyboard

import android.content.Context
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import androidx.annotation.RequiresApi

/**
 * Quick Settings Tile for Harfiy.
 *
 * Allows the user to toggle Harfiy ON and OFF in 1 SINGLE TAP directly from
 * the Android notification shade (parda / quick settings) from anywhere!
 */
@RequiresApi(Build.VERSION_CODES.N)
class HarfiyTileService : TileService() {

    override fun onStartListening() {
        super.onStartListening()
        updateTileState()
    }

    override fun onClick() {
        super.onClick()
        val isServiceActive = HarfiyAccessibilityService.instance != null

        if (isServiceActive) {
            // 1-Click: Turn Harfiy OFF directly
            HarfiyAccessibilityService.instance?.disableService()
        } else {
            // 1-Click: Open Harfiy to enable
            val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
            if (launchIntent != null) {
                launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                startActivityAndCollapse(launchIntent)
            }
        }
        updateTileState()
    }

    private fun updateTileState() {
        val tile = qsTile ?: return
        val isServiceActive = HarfiyAccessibilityService.instance != null

        if (isServiceActive) {
            tile.state = Tile.STATE_ACTIVE
            tile.label = "Harfiy: Faol"
            tile.contentDescription = "Harfiy faol (Klaviaturangiz ustida ishlamoqda)"
        } else {
            tile.state = Tile.STATE_INACTIVE
            tile.label = "Harfiy: O'chiq"
            tile.contentDescription = "Harfiy o'chiq"
        }
        tile.updateTile()
    }
}
