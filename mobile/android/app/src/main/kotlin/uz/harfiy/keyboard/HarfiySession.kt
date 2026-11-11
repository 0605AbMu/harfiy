package uz.harfiy.keyboard

/**
 * JNI bridge to the Rust edit session (`crates/ffi/src/session.rs`).
 *
 * The session decides *when* and *what* the accessibility service may write; the service
 * only forwards events and performs the reads and writes it is asked for. All offsets
 * are UTF-16 units, times are [android.os.SystemClock.uptimeMillis].
 *
 * Tick codes returned by the event methods: [TICK_IDLE], [TICK_SNAPSHOT], or a delay in
 * milliseconds after which [onTick] must be called (replacing any armed timer).
 */
object HarfiySession {

    init {
        // Loads libharfiy_ffi.so (same library as the rules engine).
        HarfiyEngine
    }

    const val TICK_IDLE = -1L
    const val TICK_SNAPSHOT = -2L

    /** Mirrors `SNAPSHOT_LEN`, `TAIL_LEN` and `VERIFY_SLACK` in session.rs. */
    const val SNAPSHOT_LEN = 256
    const val TAIL_LEN = 16
    const val VERIFY_SLACK = 3

    /** [onSnapshot] meta layout; mirrors `SNAPSHOT_META_LEN` in jni_api.rs. */
    const val META_LEN = 12
    const val META_RESULT = 0
    const val META_MODE = 1
    const val META_START = 2
    const val META_END = 3
    const val META_VERIFY_LEN = 4
    const val META_RETRY_MS = 5
    const val META_EXPECTED_LEN = 6
    const val META_KINDS = 7

    const val RESULT_DONE = 0
    const val RESULT_WRITE = 1
    const val RESULT_RETRY = 2

    const val MODE_DELETE = 2
    const val MODE_INSERT = 3

    @JvmStatic external fun configure(enableNg: Boolean, instantUndo: Boolean)
    @JvmStatic external fun startInput(enabled: Boolean, selStart: Int, selEnd: Int, now: Long)
    @JvmStatic external fun finishInput(now: Long)
    @JvmStatic external fun isFieldOn(): Boolean

    /** 1: the keyboard composes words, 0: it types key by key, -1: not enough typing yet. */
    @JvmStatic external fun keyboardMode(): Int
    @JvmStatic external fun onSelection(selStart: Int, selEnd: Int, compStart: Int, compEnd: Int, now: Long): Long
    @JvmStatic external fun onTextChanged(from: Int, removed: Int, added: Int, tail: String?, now: Long): Long
    @JvmStatic external fun onTick(now: Long): Long

    /** Returns the replacement to write (meta[[META_RESULT]] == [RESULT_WRITE]) or null. */
    @JvmStatic external fun onSnapshot(before: String, collapsed: Boolean, absCursor: Int, now: Long, outMeta: IntArray): String?

    /** False when the write could not be confirmed; the session is then off for the field. */
    @JvmStatic external fun onWritten(readback: String?, absCursor: Int, now: Long): Boolean

    /** Recent decisions, one per line, positions and lengths only (no typed text). */
    @JvmStatic external fun diagnostics(): String
}
