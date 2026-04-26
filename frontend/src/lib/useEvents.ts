import { useEffect } from '@lynx-js/react'

type EventMsg = { kind?: string; [k: string]: unknown }

/**
 * Subscribe to backend WebSocket event stream. Calls `onMessage` for each
 * decoded JSON message whose `kind` matches `kinds` (or any if `kinds` is `'*'`).
 * Closes the socket on unmount.
 */
export function useEvents(
  kinds: string[] | '*',
  onMessage: (msg: EventMsg) => void,
) {
  useEffect(() => {
    const base =
      (import.meta.env?.PUBLIC_API_BASE as string | undefined) ?? 'http://localhost:8088'
    const wsUrl = base.replace(/^http/, 'ws') + '/events/ws'
    let ws: WebSocket | null = null
    try {
      ws = new WebSocket(wsUrl)
    } catch {
      return
    }
    if (!ws) return
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(String(ev.data)) as EventMsg
        if (kinds === '*' || (msg.kind && kinds.includes(msg.kind))) {
          onMessage(msg)
        }
      } catch {}
    }
    return () => {
      try { ws?.close() } catch {}
    }
    // onMessage/kinds intentionally not in deps — stable per panel.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
}
