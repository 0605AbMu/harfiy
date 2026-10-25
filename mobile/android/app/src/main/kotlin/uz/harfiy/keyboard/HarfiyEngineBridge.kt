package uz.harfiy.keyboard

/**
 * Result returned from the Harfiy conversion engine.
 */
sealed class HarfiyAction {
    object PassThrough : HarfiyAction()
    object Suppress : HarfiyAction()
    data class Transform(val backspacesCount: Int, val replacement: String) : HarfiyAction()
}

/**
 * JNI Bridge to the high-performance Rust HarfiyEngine instance.
 */
class HarfiyEngineBridge(enableNg: Boolean = false) : AutoCloseable {

    companion object {
        init {
            try {
                System.loadLibrary("harfiy_ffi")
            } catch (e: UnsatisfiedLinkError) {
                // In local unit tests or mock preview environments
                System.err.println("Notice: harfiy_ffi native library not found: ${e.message}")
            }
        }

        @JvmStatic
        private external fun createEngine(enableNg: Boolean): Long

        @JvmStatic
        private external fun destroyEngine(handle: Long)

        @JvmStatic
        private external fun resetEngine(handle: Long)

        @JvmStatic
        private external fun setEnableNg(handle: Long, enableNg: Boolean)

        @JvmStatic
        private external fun isEnableNg(handle: Long): Boolean

        @JvmStatic
        private external fun processChar(handle: Long, unicodeChar: Int, outMeta: IntArray): String?

        @JvmStatic
        private external fun processBackspace(handle: Long, outMeta: IntArray): String?

        @JvmStatic
        private external fun convertText(
            inputText: String,
            enableNg: Boolean,
            smartApostrophe: Boolean
        ): String
    }

    private var handle: Long = 0
    private val metaBuffer = IntArray(2)

    init {
        try {
            handle = createEngine(enableNg)
        } catch (_: UnsatisfiedLinkError) {
            handle = 0
        }
    }

    override fun close() {
        if (handle != 0L) {
            destroyEngine(handle)
            handle = 0L
        }
    }

    /**
     * Reset the engine state (e.g. cursor tap, text selection, or focus change).
     */
    fun reset() {
        if (handle != 0L) {
            resetEngine(handle)
        }
    }

    /**
     * Dynamically update 'ng' rule configuration.
     */
    var isEnableNg: Boolean
        get() = if (handle != 0L) Companion.isEnableNg(handle) else false
        set(enable) {
            if (handle != 0L) {
                Companion.setEnableNg(handle, enable)
            }
        }

    /**
     * Process a typed character.
     */
    fun processChar(char: Char): HarfiyAction {
        if (handle == 0L) return HarfiyAction.PassThrough

        metaBuffer[0] = 0
        metaBuffer[1] = 0

        val replacement = processChar(handle, char.code, metaBuffer)
        val actionType = metaBuffer[0]
        val backspaces = metaBuffer[1]

        return when (actionType) {
            0 -> HarfiyAction.PassThrough
            1 -> HarfiyAction.Suppress
            2 -> HarfiyAction.Transform(backspaces, replacement ?: "")
            else -> HarfiyAction.PassThrough
        }
    }

    /**
     * Process Backspace event (supporting Instant Undo).
     */
    fun processBackspace(): HarfiyAction {
        if (handle == 0L) return HarfiyAction.PassThrough

        metaBuffer[0] = 0
        metaBuffer[1] = 0

        val replacement = processBackspace(handle, metaBuffer)
        val actionType = metaBuffer[0]
        val backspaces = metaBuffer[1]

        return when (actionType) {
            0 -> HarfiyAction.PassThrough
            1 -> HarfiyAction.Suppress
            2 -> HarfiyAction.Transform(backspaces, replacement ?: "")
            else -> HarfiyAction.PassThrough
        }
    }

    /**
     * Convert an entire text in batch mode.
     */
    fun convertText(text: String, smartApostrophe: Boolean = true): String {
        return if (handle != 0L) {
            convertText(text, isEnableNg, smartApostrophe)
        } else {
            text
        }
    }
}
