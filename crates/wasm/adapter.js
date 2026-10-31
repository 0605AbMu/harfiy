import { WasmHarfiyEngine } from './harfiy_wasm.js';

/**
 * Attaches real-time Harfiy conversion to an HTML input or textarea element.
 * Handles keystroke interception, caret preservation, and Instant Undo on Backspace.
 *
 * @param {HTMLInputElement | HTMLTextAreaElement} element
 * @param {Object} [options]
 * @param {WasmHarfiyEngine} [options.engine] Optional pre-configured engine instance
 * @param {boolean} [options.enableNg] Whether 'ng' -> 'ñ' is enabled (default: false)
 * @param {(replacement: string, trigger: string) => void} [options.onTransform] Callback on transformation
 * @returns {() => void} Cleanup function to remove event listeners
 */
export function attachHarfiy(element, options = {}) {
  const engine = options.engine || new WasmHarfiyEngine(options.enableNg ?? false, true);

  const handleKeyDown = (event) => {
    // Ignore keyboard shortcuts with modifiers (Ctrl, Cmd, Alt)
    if (event.ctrlKey || event.metaKey || event.altKey) {
      return;
    }

    // Ignore composition (IME)
    if (event.isComposing) {
      return;
    }

    const start = element.selectionStart ?? 0;
    const end = element.selectionEnd ?? 0;

    // If text is selected (range > 0), typing or backspace replaces the selection.
    if (start !== end) {
      engine.reset();
      return;
    }

    if (event.key === 'Backspace') {
      const action = engine.process_backspace();
      if (action.is_transform()) {
        event.preventDefault();
        const deleteCount = action.backspaces_count;
        const replacement = action.replacement;
        replaceText(element, start - deleteCount, start, replacement);
        if (options.onTransform) {
          options.onTransform(replacement, 'backspace_undo');
        }
      }
      return;
    }

    if (event.key.length === 1) {
      const action = engine.process_char(event.key);
      if (action.is_transform()) {
        event.preventDefault();
        const deleteCount = action.backspaces_count;
        const replacement = action.replacement;
        replaceText(element, start - deleteCount, start, replacement);
        if (options.onTransform) {
          options.onTransform(replacement, event.key);
        }
      }
      return;
    }

    // Handle navigation keys (Arrow keys, Home, End, Escape, Enter, etc.)
    engine.process_key(event.key);
  };

  const handleReset = () => {
    engine.reset();
  };

  element.addEventListener('keydown', handleKeyDown);
  element.addEventListener('click', handleReset);
  element.addEventListener('focus', handleReset);

  return () => {
    element.removeEventListener('keydown', handleKeyDown);
    element.removeEventListener('click', handleReset);
    element.removeEventListener('focus', handleReset);
  };
}

function replaceText(element, from, to, text) {
  element.focus();
  element.setSelectionRange(from, to);
  let handled = false;
  try {
    handled = document.execCommand && document.execCommand('insertText', false, text);
  } catch (_) {
    handled = false;
  }

  if (!handled) {
    element.setRangeText(text, from, to, 'end');
    element.dispatchEvent(new Event('input', { bubbles: true }));
  }
}
