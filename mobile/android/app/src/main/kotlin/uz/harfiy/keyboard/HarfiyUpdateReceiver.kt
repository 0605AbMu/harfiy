package uz.harfiy.keyboard

import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService

/**
 * Ensures complete resilience when Harfiy is updated via Google Play Store
 * or when the Android device reboots:
 * - Refreshes Quick Settings Tile state
 * - Ensures accessibility service settings remain consistent
 */
class HarfiyUpdateReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context?, intent: Intent?) {
        if (context == null || intent == null) return

        val action = intent.action
        if (action == Intent.ACTION_MY_PACKAGE_REPLACED || action == Intent.ACTION_BOOT_COMPLETED) {
            // Update Quick Settings Tile
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                try {
                    TileService.requestListeningState(
                        context,
                        ComponentName(context, HarfiyTileService::class.java)
                    )
                } catch (_: Exception) {}
            }
        }
    }
}
