import { useEffect } from '@lynx-js/react'

export type Binding = {
  /** Lowercase key (e.g. 'k', '/', '?'). Use ',' for comma. */
  key: string
  meta?: boolean
  ctrl?: boolean
  shift?: boolean
  alt?: boolean
  /** Skip when focus is in an editable element (default true). */
  ignoreInputs?: boolean
  handler: () => void
}

/**
 * Register global keyboard shortcuts on the host window. No-op on platforms
 * without DOM (Lynx native). Cleans up on unmount.
 */
export function useKeyboardShortcuts(bindings: Binding[]) {
  useEffect(() => {
    const w = globalThis as {
      addEventListener?: (e: string, fn: (ev: KeyboardEvent) => void) => void
      removeEventListener?: (e: string, fn: (ev: KeyboardEvent) => void) => void
    }
    if (!w.addEventListener) return
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as Element | null
      const isInput =
        target?.tagName === 'INPUT' ||
        target?.tagName === 'TEXTAREA' ||
        (target as HTMLElement | null)?.isContentEditable === true
      const k = e.key.toLowerCase()
      for (const b of bindings) {
        if (b.key !== k) continue
        if (b.meta !== undefined && b.meta !== e.metaKey) continue
        if (b.ctrl !== undefined && b.ctrl !== e.ctrlKey) continue
        if (b.shift !== undefined && b.shift !== e.shiftKey) continue
        if (b.alt !== undefined && b.alt !== e.altKey) continue
        if ((b.ignoreInputs ?? true) && isInput) continue
        e.preventDefault()
        b.handler()
        return
      }
    }
    w.addEventListener('keydown', onKey)
    return () => w.removeEventListener?.('keydown', onKey)
  }, [bindings])
}
