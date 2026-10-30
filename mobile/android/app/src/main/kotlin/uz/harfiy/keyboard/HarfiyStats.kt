package uz.harfiy.keyboard

import android.content.Context
import android.content.SharedPreferences
import android.os.Handler
import android.os.Looper
import androidx.core.content.edit

/**
 * Conversion counters, treated as secondary data.
 *
 * Counting happens in memory on the typing path; disk writes are batched and deferred.
 * Losing the last few counts when the process is killed is an accepted trade-off.
 * Kept in their own preferences file so a settings write never rewrites counters and
 * vice versa. All calls are expected on the main thread.
 */
object HarfiyStats {

    private const val PREFS_NAME = "harfiy_stats"
    private const val FLUSH_DELAY_MS = 30_000L

    private val keys = HarfiyEngine.RuleKind.entries.map { "stats_" + it.name.lowercase() }

    private val pending = LongArray(keys.size)
    private val mainHandler = Handler(Looper.getMainLooper())
    private var prefs: SharedPreferences? = null
    private val flushRunnable = Runnable { flush() }

    private fun prefs(context: Context): SharedPreferences =
        prefs ?: context.applicationContext
            .getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            .also { prefs = it }

    fun record(context: Context, kind: HarfiyEngine.RuleKind) {
        prefs(context)
        pending[kind.ordinal]++
        // Debounced from the first unflushed count, so a long typing burst is one write.
        if (!mainHandler.hasCallbacks(flushRunnable)) {
            mainHandler.postDelayed(flushRunnable, FLUSH_DELAY_MS)
        }
    }

    /** Persisted plus not-yet-flushed counts, indexed by [HarfiyEngine.RuleKind.ordinal]. */
    fun snapshot(context: Context): LongArray {
        val stored = prefs(context)
        return LongArray(keys.size) { stored.getLong(keys[it], 0L) + pending[it] }
    }

    fun reset(context: Context) {
        mainHandler.removeCallbacks(flushRunnable)
        pending.fill(0L)
        prefs(context).edit { clear() }
    }

    fun flush() {
        mainHandler.removeCallbacks(flushRunnable)
        val stored = prefs ?: return
        if (pending.all { it == 0L }) return
        stored.edit {
            for (i in keys.indices) {
                if (pending[i] != 0L) putLong(keys[i], stored.getLong(keys[i], 0L) + pending[i])
            }
        }
        pending.fill(0L)
    }
}
