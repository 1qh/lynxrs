import type { ReactNode } from '@lynx-js/react'
import { MemoryRouter } from 'react-router'
import { HashRouter } from 'react-router-dom'

/**
 * Cross-platform router. Picks `HashRouter` when window.location is available
 * (rspeedy web target, browsers, Lynx web preview); `MemoryRouter` otherwise
 * (native Lynx runtime where DOM APIs aren't present). HashRouter — not
 * BrowserRouter — so deep links survive a static-file host without backend
 * rewrites and so the rspeedy preview iframe doesn't fight server routing.
 */
export function CrossRouter({ children }: { children: ReactNode }) {
  const hasLocation =
    typeof globalThis !== 'undefined' &&
    typeof (globalThis as { location?: { hash: string } }).location !== 'undefined'
  if (hasLocation) {
    return <HashRouter>{children}</HashRouter>
  }
  return <MemoryRouter>{children}</MemoryRouter>
}
