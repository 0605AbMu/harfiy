#ifndef HARFIY_H
#define HARFIY_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Opaque pointer to the Harfiy conversion engine.
typedef void* HarfiyEngineHandle;

/// Actions returned after processing a key event.
typedef enum {
    HARFIY_ACTION_PASS_THROUGH = 0,
    HARFIY_ACTION_SUPPRESS = 1,
    HARFIY_ACTION_TRANSFORM = 2
} HarfiyActionType;

/// Detailed transformation result.
/// For HARFIY_ACTION_TRANSFORM, `replacement_utf8` points to `replacement_len` bytes of UTF-8
/// (e.g. "ş", "ç", "ö", "ğ"). The bytes are NOT NUL-terminated and live for the whole process:
/// do not free them. For any other action the pointer is NULL and the length is 0.
typedef struct {
    HarfiyActionType action_type;
    uint32_t backspaces_count;
    const uint8_t* replacement_utf8;
    size_t replacement_len;
} HarfiyActionResult;

/// Create a new Harfiy engine instance.
HarfiyEngineHandle harfiy_engine_create(bool enable_ng);

/// Destroy a Harfiy engine instance.
void harfiy_engine_destroy(HarfiyEngineHandle handle);

/// Reset engine state back to Idle (e.g. on cursor move, space, or app switch).
void harfiy_engine_reset(HarfiyEngineHandle handle);

/// Dynamically update 'ng' -> 'ñ' rule status.
void harfiy_engine_set_enable_ng(HarfiyEngineHandle handle, bool enable_ng);

/// Check if 'ng' rule is enabled.
bool harfiy_engine_is_enable_ng(HarfiyEngineHandle handle);

/// Process a single Unicode scalar character (e.g. from key tap).
HarfiyActionResult harfiy_engine_process_char(HarfiyEngineHandle handle, uint32_t unicode_char);

/// Process a Backspace event (handles Instant Undo).
HarfiyActionResult harfiy_engine_process_backspace(HarfiyEngineHandle handle);

/// Process a word delimiter (e.g. space, punctuation) which resets the buffer.
void harfiy_engine_process_delimiter(HarfiyEngineHandle handle);

/// Convert an entire string to the new Uzbek Latin orthography in batch mode.
/// Returns a heap-allocated UTF-8 string that must be freed using `harfiy_free_string`.
char* harfiy_convert_text(
    const char* input_utf8,
    bool enable_ng,
    bool smart_apostrophe,
    uint32_t* out_replacements_count
);

/// Free a string returned by `harfiy_convert_text`.
void harfiy_free_string(char* s);

#ifdef __cplusplus
}
#endif

#endif // HARFIY_H
