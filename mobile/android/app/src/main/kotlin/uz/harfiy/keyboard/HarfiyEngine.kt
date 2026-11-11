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
     * Rule kinds counted in usage statistics; the ordinals mirror `RuleKind` in the
     * engine crate.
     */
    enum class RuleKind { SH, CH, O, G, NG }

    /** Converts a whole text in batch mode. */
    fun convert(text: String, enableNg: Boolean, smartApostrophe: Boolean = true): String =
        convertText(text, enableNg, smartApostrophe)

    @JvmStatic
    private external fun convertText(inputText: String, enableNg: Boolean, smartApostrophe: Boolean): String
}
