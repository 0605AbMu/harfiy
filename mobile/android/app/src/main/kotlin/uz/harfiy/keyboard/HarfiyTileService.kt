package uz.harfiy.keyboard

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import androidx.annotation.RequiresApi

/**
 * Quick Settings Tile for Harfiy.
 *
 * Allows the user to toggle Harfiy ON and OFF (Pause / Resume) in 1 SINGLE TAP directly from
 * the Android notification shade (parda / quick settings) from anywhere without leaving their current app!
 */
@RequiresApi(Build.VERSION_CODES.N)
class HarfiyTileService : TileService() {

    companion object {
        fun requestTileUpdate(context: Context) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                try {
                    requestListeningState(
                        context.applicationContext,
                        ComponentName(context.applicationContext, HarfiyTileService::class.java)
                    )
                } catch (_: Exception) {
                }
            }
        }
    }

    override fun onStartListening() {
        super.onStartListening()
        updateTileState()
    }

    override fun onClick() {
        super.onClick()
        val isServiceActive = HarfiyAccessibilityService.instance != null
        val prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)

        if (isServiceActive) {
            // Service is active in system: 1-Click to toggle PAUSE / RESUME without unbinding!
            val currentlyPaused = prefs.getBoolean(HarfiyAccessibilityService.KEY_IS_PAUSED, false)
            val newPaused = !currentlyPaused
            prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_IS_PAUSED, newPaused).apply()
            updateTileState()
        } else {
            // Service is not enabled in Android settings: Launch app to guide activation
            val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
            if (launchIntent != null) {
                launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                startActivityAndCollapse(launchIntent)
            }
        }
    }

    private fun updateTileState() {
        val tile = qsTile ?: return
        val isServiceActive = HarfiyAccessibilityService.instance != null
        val prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)
        val isPaused = prefs.getBoolean(HarfiyAccessibilityService.KEY_IS_PAUSED, false)

        // Use the official Harfiy App Icon for the Quick Settings tile
        tile.icon = android.graphics.drawable.Icon.createWithResource(this, R.mipmap.ic_launcher)

        if (isServiceActive && !isPaused) {
            tile.state = Tile.STATE_ACTIVE
            tile.label = "Harfiy: Faol"
            tile.contentDescription = "Harfiy faol va harflarni o‘girmoqda"
        } else if (isServiceActive && isPaused) {
            tile.state = Tile.STATE_INACTIVE
            tile.label = "Harfiy: Pauza"
            tile.contentDescription = "Harfiy vaqtinchalik to‘xtatilgan"
        } else {
            tile.state = Tile.STATE_INACTIVE
            tile.label = "Harfiy: O‘chiq"
            tile.contentDescription = "Harfiy tizimda yoqilmagan"
        }
        tile.updateTile()
    }
}
