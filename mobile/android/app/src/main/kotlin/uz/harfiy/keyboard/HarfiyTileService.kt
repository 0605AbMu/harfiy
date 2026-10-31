package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityServiceInfo
import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.view.accessibility.AccessibilityManager
import androidx.annotation.RequiresApi
import androidx.core.content.edit

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

    @SuppressLint("ApplySharedPref", "StartActivityAndCollapseDeprecated")
    override fun onClick() {
        super.onClick()
        val isServiceActive = isAccessibilityServiceEnabled()
        val prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)

        if (isServiceActive) {
            // Service is active in system: 1-Click to toggle PAUSE / RESUME without unbinding!
            val currentlyPaused = prefs.getBoolean(HarfiyAccessibilityService.KEY_IS_PAUSED, false)
            val newPaused = !currentlyPaused
            prefs.edit(commit = true) { putBoolean(HarfiyAccessibilityService.KEY_IS_PAUSED, newPaused) }
            updateTileState()
        } else {
            // Service is not enabled in Android settings: Launch app to guide activation
            val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
            if (launchIntent != null) {
                launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                    val pendingIntent = PendingIntent.getActivity(
                        this,
                        0,
                        launchIntent,
                        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
                    )
                    startActivityAndCollapse(pendingIntent)
                } else {
                    @Suppress("DEPRECATION")
                    startActivityAndCollapse(launchIntent)
                }
            }
        }
    }

    private fun isAccessibilityServiceEnabled(): Boolean {
        if (HarfiyAccessibilityService.instance != null) {
            return true
        }

        val am = getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager ?: return false
        val list = am.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK)
        return list.any {
            it.resolveInfo.serviceInfo.packageName == packageName &&
            it.resolveInfo.serviceInfo.name.contains("HarfiyAccessibilityService")
        }
    }

    private fun updateTileState() {
        val tile = qsTile ?: return
        val isServiceActive = isAccessibilityServiceEnabled()
        val prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)
        val isPaused = prefs.getBoolean(HarfiyAccessibilityService.KEY_IS_PAUSED, false)

        // Use the official Harfiy Brand Logo with transparent background (properly tinted by Android)
        tile.icon = Icon.createWithResource(this, R.drawable.ic_tile_harfiy)

        if (isServiceActive && !isPaused) {
            tile.state = Tile.STATE_ACTIVE
            tile.label = "Harfiy: Faol"
            tile.contentDescription = "Harfiy faol va harflarni o‘girmoqda"
        } else if (isServiceActive) {
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
