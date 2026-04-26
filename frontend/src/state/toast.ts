import { create } from 'zustand'

export type Toast = { id: number; kind: 'error' | 'info'; text: string }

interface ToastState {
  toasts: Toast[]
  push: (kind: Toast['kind'], text: string) => void
  dismiss: (id: number) => void
}

let nextId = 1

const AUTO_DISMISS_MS = 5_000

export const useToasts = create<ToastState>((set, get) => ({
  toasts: [],
  push: (kind, text) => {
    const id = nextId++
    set((s) => ({ toasts: [...s.toasts, { id, kind, text }] }))
    const t = (globalThis as { setTimeout?: (f: () => void, ms: number) => unknown }).setTimeout
    if (t) t(() => get().dismiss(id), AUTO_DISMISS_MS)
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}))

/** Surface an API error to the user. Pass the openapi-client `error` field. */
export function reportError(err: unknown, fallback = 'Request failed') {
  const msg =
    (err && typeof err === 'object' && 'message' in err && typeof err.message === 'string'
      ? err.message
      : null) ?? fallback
  useToasts.getState().push('error', msg)
}
