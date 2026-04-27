import { create } from 'zustand'

export type Theme = 'light' | 'dark'

interface ThemeState {
  theme: Theme
  toggle: () => void
  set: (t: Theme) => void
}

function readInitial(): Theme {
  try {
    const v = (globalThis as { localStorage?: Storage }).localStorage?.getItem('simu.theme')
    if (v === 'light' || v === 'dark') return v
    const m = (globalThis as { matchMedia?: (q: string) => { matches: boolean } }).matchMedia
    if (m && m('(prefers-color-scheme: dark)').matches) return 'dark'
  } catch {}
  return 'light'
}

function apply(theme: Theme) {
  try {
    const doc = (globalThis as { document?: Document }).document
    if (!doc) return
    const root = doc.documentElement
    if (theme === 'dark') root.classList.add('dark')
    else root.classList.remove('dark')
  } catch {}
}

const initial = readInitial()
apply(initial)

export const useTheme = create<ThemeState>((set, get) => ({
  theme: initial,
  toggle: () => {
    const next: Theme = get().theme === 'light' ? 'dark' : 'light'
    apply(next)
    try {
      ;(globalThis as { localStorage?: Storage }).localStorage?.setItem('simu.theme', next)
    } catch {}
    set({ theme: next })
  },
  set: (t) => {
    apply(t)
    try {
      ;(globalThis as { localStorage?: Storage }).localStorage?.setItem('simu.theme', t)
    } catch {}
    set({ theme: t })
  },
}))
