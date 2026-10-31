import { WasmHarfiyEngine } from './harfiy_wasm';

export interface AttachOptions {
  engine?: WasmHarfiyEngine;
  enableNg?: boolean;
  onTransform?: (replacement: string, trigger: string) => void;
}

/**
 * Attaches real-time Harfiy conversion to an HTML input or textarea element.
 * Handles keystroke interception, caret preservation, and Instant Undo on Backspace.
 *
 * @param element HTMLInputElement or HTMLTextAreaElement
 * @param options Configuration options
 * @returns Cleanup function to remove event listeners
 */
export function attachHarfiy(
  element: HTMLInputElement | HTMLTextAreaElement,
  options?: AttachOptions
): () => void;
