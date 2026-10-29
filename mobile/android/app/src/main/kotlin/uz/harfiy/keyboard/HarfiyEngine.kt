package uz.harfiy.keyboard

/**
 * JNI bridge to the Rust Harfiy engine: the only source of conversion rules on Android.
 *
 * The native library is mandatory. A missing or broken `libharfiy_ffi.so` must fail
 * loudly on first use instead of degrading into a silent no-op.
 */
object HarfiyEngine {

    init {
        System.loadLibrary("harfiy_ffi")
    }

    /**
     * Rule kinds reported by [planTail]; the ordinals mirror `RuleKind` in the
     * engine crate.
     */
    enum class RuleKind { SH, CH, O, G, NG }

    /** Replace the [deleteCount] characters right before the cursor with [replacement]. */
    class TailEdit(val deleteCount: Int, val replacement: String, val kind: RuleKind)

    private val ruleKinds = RuleKind.entries.toTypedArray()

    // Only touched from the accessibility service's main thread.
    private val metaBuffer = IntArray(2)

    /**
     * Plans the conversion triggered by [curr] having just been typed after [prev]
     * (null when [curr] starts the field), or returns null when nothing converts.
     */
    fun planTail(prev: Char?, curr: Char, enableNg: Boolean): TailEdit? {
        val replacement = planTail(prev?.code ?: -1, curr.code, enableNg, metaBuffer) ?: return null
        return TailEdit(metaBuffer[0], replacement, ruleKinds[metaBuffer[1]])
    }

    /** Replace `text[start until end]` with [replacement]; [kind] is null for non-letter edits. */
    class Edit(val start: Int, val end: Int, val replacement: String, val kind: RuleKind?)

    /**
     * Plans every live-typing edit in [text] (no smart apostrophe), in ascending,
     * non-overlapping order. Offsets index [text] as a Kotlin string.
     */
    fun planEdits(text: String, enableNg: Boolean): List<Edit> {
        val flat = planEditsNative(text, enableNg)
        return List(flat.size / EDIT_STRIDE) { i ->
            val base = i * EDIT_STRIDE
            Edit(
                start = flat[base],
                end = flat[base + 1],
                replacement = String(Character.toChars(flat[base + 3])),
                kind = flat[base + 2].let { if (it < 0) null else ruleKinds[it] }
            )
        }
    }

    /** Converts a whole text in batch mode. */
    fun convert(text: String, enableNg: Boolean, smartApostrophe: Boolean = true): String =
        convertText(text, enableNg, smartApostrophe)

    @JvmStatic
    private external fun planTail(prev: Int, curr: Int, enableNg: Boolean, outMeta: IntArray): String?

    // Ints per edit in planEditsNative's result; mirrors EDIT_STRIDE in the ffi crate.
    private const val EDIT_STRIDE = 4

    @JvmStatic
    private external fun planEditsNative(text: String, enableNg: Boolean): IntArray

    @JvmStatic
    private external fun convertText(inputText: String, enableNg: Boolean, smartApostrophe: Boolean): String
}
