import { useCallback, useEffect, useRef, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { reportError } from '../../state/toast.js'

type Message = {
  id: string
  role: 'system' | 'user' | 'assistant' | 'tool'
  content: string
  created_at: string
}
type Conversation = {
  id: string
  title: string
  model: string
  updated_at: string
}

const BASE =
  (import.meta.env?.PUBLIC_API_BASE as string | undefined) ?? 'http://localhost:8088'

async function csrfHeaders(): Promise<Record<string, string>> {
  const doc = (globalThis as { document?: Document }).document
  const m = doc?.cookie?.match(/(?:^|;\s*)simu_csrf=([^;]+)/)
  return m ? { 'x-csrf-token': decodeURIComponent(m[1]!) } : {}
}

export function ChatPanel() {
  const { t } = useTranslation()
  const [convs, setConvs] = useState<Conversation[]>([])
  const [activeId, setActiveId] = useState<string | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [streaming, setStreaming] = useState(false)
  const inputRef = useRef('')
  const abortRef = useRef<AbortController | null>(null)
  const lastUserContentRef = useRef('')

  // Heuristic token count: ~4 chars/token. Good enough for a UX badge;
  // exact counts come from the model provider on completion.
  const tokenCount = (s: string) => Math.ceil(s.length / 4)
  const totalTokens = messages.reduce((n, m) => n + tokenCount(m.content), 0)

  const refreshConvs = useCallback(async () => {
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const r = await f(`${BASE}/api/conversations`, { credentials: 'include' })
      if (!r.ok) return
      const d = (await r.json()) as Conversation[]
      setConvs(d)
      if (!activeId && d.length > 0) setActiveId(d[0]!.id)
    } catch (e) {
      reportError(e, 'Load conversations failed')
    }
  }, [activeId])

  const loadMessages = useCallback(async (id: string) => {
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const r = await f(`${BASE}/api/conversations/${id}/messages`, { credentials: 'include' })
      if (!r.ok) return
      setMessages((await r.json()) as Message[])
    } catch (e) {
      reportError(e, 'Load messages failed')
    }
  }, [])

  useEffect(() => { void refreshConvs() }, [refreshConvs])
  useEffect(() => { if (activeId) void loadMessages(activeId) }, [activeId, loadMessages])

  const newConversation = useCallback(async () => {
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const headers = await csrfHeaders()
      const r = await f(`${BASE}/api/conversations`, {
        method: 'POST',
        credentials: 'include',
        headers: { ...headers, 'content-type': 'application/json' },
        body: JSON.stringify({ title: 'New chat' }),
      })
      if (!r.ok) {
        reportError(`status ${r.status}`, 'Create conversation failed')
        return
      }
      const c = (await r.json()) as Conversation
      setConvs((cur) => [c, ...cur])
      setActiveId(c.id)
      setMessages([])
    } catch (e) {
      reportError(e, 'Create conversation threw')
    }
  }, [])

  const send = useCallback(async (overrideText?: string) => {
    const text = (overrideText ?? inputRef.current).trim()
    if (!text || !activeId || streaming) return
    lastUserContentRef.current = text
    setStreaming(true)
    inputRef.current = ''
    // Optimistic user bubble.
    const tempId = `tmp-${Date.now()}`
    setMessages((cur) => [
      ...cur,
      { id: tempId, role: 'user', content: text, created_at: new Date().toISOString() },
    ])
    // Optimistic assistant placeholder we'll fill from SSE deltas.
    const asstTempId = `${tempId}-a`
    setMessages((cur) => [
      ...cur,
      { id: asstTempId, role: 'assistant', content: '', created_at: new Date().toISOString() },
    ])
    const ctrl = new AbortController()
    abortRef.current = ctrl
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) throw new Error('fetch unavailable')
      const headers = await csrfHeaders()
      const r = await f(`${BASE}/api/conversations/${activeId}/messages`, {
        method: 'POST',
        credentials: 'include',
        signal: ctrl.signal,
        headers: { ...headers, 'content-type': 'application/json', accept: 'text/event-stream' },
        body: JSON.stringify({ content: text }),
      })
      if (!r.ok || !r.body) throw new Error(`stream status ${r.status}`)
      const reader = r.body.getReader()
      const dec = new TextDecoder()
      let buf = ''
      let curEvent = ''
      while (true) {
        const { value, done } = await reader.read()
        if (done) break
        buf += dec.decode(value, { stream: true })
        const lines = buf.split('\n')
        buf = lines.pop() ?? ''
        for (const line of lines) {
          if (line.startsWith('event:')) {
            curEvent = line.slice(6).trim()
          } else if (line.startsWith('data:')) {
            const data = line.slice(5).trimStart()
            if (curEvent === 'delta') {
              setMessages((cur) =>
                cur.map((m) =>
                  m.id === asstTempId ? { ...m, content: m.content + data } : m,
                ),
              )
            }
            curEvent = ''
          }
        }
      }
      // Reload canonical messages once stream ends so ids match server.
      void loadMessages(activeId)
      void refreshConvs()
    } catch (e) {
      // AbortError is the user pressing Stop — not an error.
      if (!(e instanceof DOMException && e.name === 'AbortError')) {
        reportError(e, 'Stream failed')
      }
    } finally {
      abortRef.current = null
      setStreaming(false)
    }
  }, [activeId, streaming, loadMessages, refreshConvs])

  const stop = useCallback(() => {
    abortRef.current?.abort()
  }, [])

  const regenerate = useCallback(() => {
    if (streaming || !lastUserContentRef.current) return
    void send(lastUserContentRef.current)
  }, [streaming, send])

  const active = convs.find((c) => c.id === activeId) ?? null

  return (
    <view className="gap-3">
      {/* Conversation list strip */}
      <view className="flex-row items-center gap-2 overflow-auto">
        <view
          className="h-8 rounded-md bg-primary items-center justify-center px-3"
          bindtap={() => void newConversation()}
          aria-label={t('chat.new_conversation')}
        >
          <text className="text-primary-foreground text-sm font-medium">+ {t('chat.new')}</text>
        </view>
        {convs.map((c) => (
          <view
            key={c.id}
            className={
              activeId === c.id
                ? 'h-8 rounded-md bg-secondary border border-primary items-center justify-center px-3'
                : 'h-8 rounded-md bg-secondary items-center justify-center px-3'
            }
            bindtap={() => setActiveId(c.id)}
            aria-label={c.title || 'Untitled'}
          >
            <text className="text-secondary-foreground text-sm">
              {c.title || c.id.slice(0, 8)}
            </text>
          </view>
        ))}
      </view>

      {!active ? (
        <view className="rounded-md border border-dashed border-border p-6 items-center gap-2">
          <text className="text-foreground text-base font-medium">{t('chat.empty_title')}</text>
          <text className="text-sm text-muted-foreground text-center">
            {t('chat.empty_hint')}
          </text>
        </view>
      ) : (
        <view className="gap-2">
          {messages.map((m) => (
            <view
              key={m.id}
              className={
                m.role === 'user'
                  ? 'self-end max-w-[85%] rounded-md bg-primary px-3 py-2'
                  : 'self-start max-w-[85%] rounded-md bg-card border border-border px-3 py-2'
              }
            >
              {m.role === 'assistant' ? (
                <text className="text-foreground text-sm whitespace-pre-wrap">
                  {m.content || (streaming ? '…' : '')}
                </text>
              ) : (
                <text
                  className="text-primary-foreground text-sm whitespace-pre-wrap"
                >
                  {m.content}
                </text>
              )}
            </view>
          ))}
          {messages.length === 0 ? (
            <text className="text-sm text-muted-foreground text-center py-4">
              {t('chat.start_conversation')}
            </text>
          ) : null}
        </view>
      )}

      {/* Action row: stop / regenerate + token meter */}
      {active ? (
        <view className="flex-row items-center justify-between gap-2 px-1">
          <view className="flex-row gap-2">
            {streaming ? (
              <view
                className="h-7 rounded-md bg-destructive items-center justify-center px-3"
                bindtap={stop}
                aria-label={t('chat.stop')}
              >
                <text className="text-destructive-foreground text-xs font-medium">
                  ⏹ {t('chat.stop')}
                </text>
              </view>
            ) : messages.some((m) => m.role === 'assistant') ? (
              <view
                className="h-7 rounded-md bg-secondary items-center justify-center px-3"
                bindtap={regenerate}
                aria-label={t('chat.regenerate')}
              >
                <text className="text-secondary-foreground text-xs font-medium">
                  ↻ {t('chat.regenerate')}
                </text>
              </view>
            ) : null}
          </view>
          <text className="text-[11px] text-muted-foreground">
            ~{totalTokens} {t('chat.tokens')}
          </text>
        </view>
      ) : null}

      {/* Composer */}
      <view className="flex-row items-end gap-2 pt-2 border-t border-border">
        <input
          className="flex-1 h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
          placeholder={t('chat.compose_placeholder')}
          type="text"
          bindinput={(e: { detail: { value: string } }) => {
            inputRef.current = e.detail.value
          }}
        />
        <view
          className={
            streaming || !active
              ? 'h-10 rounded-md bg-muted items-center justify-center px-4'
              : 'h-10 rounded-md bg-primary items-center justify-center px-4'
          }
          bindtap={streaming || !active ? undefined : () => void send()}
          aria-label={t('chat.send')}
        >
          <text
            className={
              streaming || !active
                ? 'text-muted-foreground text-sm font-medium'
                : 'text-primary-foreground text-sm font-medium'
            }
          >
            {streaming ? '…' : t('chat.send')}
          </text>
        </view>
      </view>
    </view>
  )
}

