import type { ReactNode } from '@lynx-js/react'
import { MemoryRouter } from 'react-router'
import { HashRouter } from 'react-router-dom'

/**
 * Cross-platform router. HashRouter when both `window.location` AND
 * `document.defaultView` are present (real browser tab); MemoryRouter
 * everywhere else — including the Lynx srcdoc iframe, which has document
 * but no `defaultView`, which trips react-router-dom's createHashHistory
 * with "Cannot read properties of undefined (reading 'defaultView')".
 */
export function CrossRouter({ children }: { children: ReactNode }) {
  let canUseHash = false
  try {
    const g = globalThis as {
      location?: { hash?: string }
      document?: { defaultView?: unknown }
    }
    canUseHash = !!g.location && !!g.document?.defaultView
  } catch {}
  if (canUseHash) return <HashRouter>{children}</HashRouter>
  return <MemoryRouter>{children}</MemoryRouter>
}
