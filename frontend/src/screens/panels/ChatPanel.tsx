import { useCallback, useEffect, useRef, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { reportError } from '../../state/toast.js'
import { useEvents } from '../../lib/useEvents.js'
import { LynxMarkdown } from '../../lib/lynxMarkdown.js'
import { useProjects } from '../../state/projects.js'

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
  system_prompt?: string
  temperature?: number
  shared?: boolean
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
  const [searchQ, setSearchQ] = useState('')
  const [searchHits, setSearchHits] = useState<Array<{ conversation_id: string; title: string; snippet: string }>>([])
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [artifactsOpen, setArtifactsOpen] = useState(false)
  const projects = useProjects((s) => s.projects)
  const activeProject = useProjects((s) => s.active)
  const byConv = useProjects((s) => s.byConv)
  const setActiveProject = useProjects((s) => s.setActive)
  const addProject = useProjects((s) => s.add)
  const assignProj = useProjects((s) => s.assign)
  const filteredConvs = activeProject === 'all'
    ? convs
    : convs.filter((c) => byConv[c.id] === activeProject)

  // Parse fenced code blocks from all assistant messages → artifact list.
  const artifacts = (() => {
    const out: Array<{ lang: string; code: string }> = []
    for (const m of messages) {
      if (m.role !== 'assistant') continue
      const re = /```(\w*)\n([\s\S]*?)```/g
      let match: RegExpExecArray | null
      while ((match = re.exec(m.content)) !== null) {
        out.push({ lang: match[1] || 'text', code: match[2]! })
      }
    }
    return out
  })()
  const inputRef = useRef('')
  const searchRef = useRef('')
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

  // Multi-device live sync: when another tab/session adds a message to the
  // currently-open conversation, refresh — but only if we're not the one
  // streaming (to avoid clobbering optimistic state).
  useEvents(['message_created'], (msg) => {
    const m = msg as { conversation_id?: string }
    if (!streaming && activeId && m.conversation_id === activeId) {
      void loadMessages(activeId)
    }
    void refreshConvs()
  })

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

  const handleSlash = useCallback(async (raw: string) => {
    const cmd = raw.slice(1).trim().toLowerCase()
    if (cmd === 'clear') { setMessages([]); return true }
    if (cmd === 'export' && activeId) {
      try {
        const f = (globalThis as { fetch?: typeof fetch }).fetch
        if (!f) return true
        const r = await f(`${BASE}/api/conversations/${activeId}/export`, {
          credentials: 'include',
        })
        if (!r.ok) return true
        const d = (await r.json()) as { markdown: string }
        const doc = (globalThis as { document?: Document }).document
        if (doc) {
          const blob = new Blob([d.markdown], { type: 'text/markdown' })
          const a = doc.createElement('a')
          a.href = URL.createObjectURL(blob)
          a.download = `conversation-${activeId.slice(0, 8)}.md`
          a.click()
          URL.revokeObjectURL(a.href)
        }
      } catch (e) {
        reportError(e, 'Export failed')
      }
      return true
    }
    if (cmd === 'help') {
      setMessages((cur) => [
        ...cur,
        {
          id: `slash-${Date.now()}`,
          role: 'assistant',
          content: 'Slash commands:\n  /clear — clear visible messages\n  /export — download as markdown\n  /help — this help',
          created_at: new Date().toISOString(),
        },
      ])
      return true
    }
    return false
  }, [activeId])

  const runSearch = useCallback(async () => {
    const q = searchRef.current.trim()
    if (q.length < 2) { setSearchHits([]); return }
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const r = await f(`${BASE}/api/conversations/search?q=${encodeURIComponent(q)}`, {
        credentials: 'include',
      })
      if (!r.ok) return
      setSearchHits(await r.json())
    } catch {}
  }, [])

  const speakLast = useCallback(() => {
    const w = globalThis as { speechSynthesis?: { speak: (u: unknown) => void; cancel: () => void } }
    const Ctor = (globalThis as { SpeechSynthesisUtterance?: { new (s: string): unknown } }).SpeechSynthesisUtterance
    if (!w.speechSynthesis || !Ctor) return
    const last = [...messages].reverse().find((m) => m.role === 'assistant')
    if (!last) return
    w.speechSynthesis.cancel()
    const u = new Ctor(last.content)
    w.speechSynthesis.speak(u)
  }, [messages])

  const attachFile = useCallback(() => {
    const doc = (globalThis as { document?: Document }).document
    if (!doc) return
    const el = doc.createElement('input')
    el.type = 'file'
    el.onchange = async () => {
      const f = el.files?.[0]
      if (!f) return
      const isImage = f.type.startsWith('image/')
      const isText = f.type.startsWith('text/') || /\.(md|json|csv|tsv|log|ya?ml|toml|conf|sql|sh|js|ts|tsx|jsx|rs|py|go|rb|java|c|h|cpp|hpp)$/i.test(f.name)
      let injected: string
      if (isImage) {
        const ab = await f.arrayBuffer()
        const bytes = new Uint8Array(ab)
        let bin = ''
        for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!)
        const dataUrl = `data:${f.type || 'image/png'};base64,${btoa(bin)}`
        injected = `![${f.name}](${dataUrl})`
      } else if (isText) {
        const text = await f.text()
        // Trim aggressively so prompts don't blow context limits; warn user.
        const max = 8000
        const body = text.length > max ? text.slice(0, max) + '\n…(truncated)' : text
        injected = `\n\n\`\`\`\n# ${f.name}\n${body}\n\`\`\`\n`
      } else {
        injected = `[file: ${f.name} · ${f.type || 'binary'} · ${f.size}B (binary attachments not yet supported in chat)]`
      }
      inputRef.current = `${inputRef.current}\n\n${injected}`
      const input = doc.querySelector('input[placeholder]') as HTMLInputElement | null
      if (input) {
        input.value = inputRef.current
        input.dispatchEvent(new Event('input', { bubbles: true }))
      }
    }
    el.click()
  }, [])

  const shareConversation = useCallback(async () => {
    if (!activeId) return
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const headers = await csrfHeaders()
      const r = await f(`${BASE}/api/conversations/${activeId}/share`, {
        method: 'POST',
        credentials: 'include',
        headers,
      })
      if (!r.ok) {
        reportError(`status ${r.status}`, 'Share failed')
        return
      }
      const d = (await r.json()) as { url: string; token: string }
      const w = globalThis as { prompt?: (m: string, def?: string) => string | null }
      w.prompt?.('Public share URL (copy):', d.url)
      void refreshConvs()
    } catch (e) {
      reportError(e, 'Share threw')
    }
  }, [activeId, refreshConvs])

  const updateSettings = useCallback(
    async (patch: { system_prompt?: string; temperature?: number; model?: string }) => {
      if (!activeId) return
      try {
        const f = (globalThis as { fetch?: typeof fetch }).fetch
        if (!f) return
        const headers = await csrfHeaders()
        await f(`${BASE}/api/conversations/${activeId}`, {
          method: 'PATCH',
          credentials: 'include',
          headers: { ...headers, 'content-type': 'application/json' },
          body: JSON.stringify(patch),
        })
        void refreshConvs()
      } catch (e) {
        reportError(e, 'Save settings failed')
      }
    },
    [activeId, refreshConvs],
  )

  const startVoice = useCallback(() => {
    const w = globalThis as {
      SpeechRecognition?: { new (): any }
      webkitSpeechRecognition?: { new (): any }
      document?: Document
    }
    const Ctor = w.SpeechRecognition ?? w.webkitSpeechRecognition
    if (!Ctor) return
    const rec = new Ctor()
    rec.lang = 'en-US'
    rec.interimResults = false
    rec.maxAlternatives = 1
    rec.onresult = (e: { results: ArrayLike<{ 0: { transcript: string } }> }) => {
      const transcript = (e.results[0] as unknown as { 0: { transcript: string } })[0].transcript
      inputRef.current = transcript
      // Best-effort: write into the visible input field (Lynx reads on bindinput).
      const doc = w.document
      const el = doc?.querySelector('input[type="text"][placeholder]') as HTMLInputElement | null
      if (el) {
        el.value = transcript
        el.dispatchEvent(new Event('input', { bubbles: true }))
      }
    }
    try { rec.start() } catch {}
  }, [])

  const renameConversation = useCallback(async (id: string) => {
    const w = globalThis as { prompt?: (msg: string, def?: string) => string | null }
    if (!w.prompt) return
    const cur = convs.find((c) => c.id === id)
    const next = w.prompt('Conversation title', cur?.title ?? '')
    if (next == null) return
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const headers = await csrfHeaders()
      await f(`${BASE}/api/conversations/${id}`, {
        method: 'PATCH',
        credentials: 'include',
        headers: { ...headers, 'content-type': 'application/json' },
        body: JSON.stringify({ title: next }),
      })
      void refreshConvs()
    } catch (e) {
      reportError(e, 'Rename failed')
    }
  }, [convs, refreshConvs])

  const deleteConversation = useCallback(async (id: string) => {
    const w = globalThis as { confirm?: (msg: string) => boolean }
    if (w.confirm && !w.confirm('Delete this conversation?')) return
    try {
      const f = (globalThis as { fetch?: typeof fetch }).fetch
      if (!f) return
      const headers = await csrfHeaders()
      await f(`${BASE}/api/conversations/${id}`, {
        method: 'DELETE',
        credentials: 'include',
        headers,
      })
      if (activeId === id) {
        setActiveId(null)
        setMessages([])
      }
      void refreshConvs()
    } catch (e) {
      reportError(e, 'Delete failed')
    }
  }, [activeId, refreshConvs])

  const send = useCallback(async (overrideText?: string) => {
    const text = (overrideText ?? inputRef.current).trim()
    if (!text || !activeId || streaming) return
    if (text.startsWith('/')) {
      const handled = await handleSlash(text)
      if (handled) {
        inputRef.current = ''
        return
      }
    }
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
      {/* Search */}
      <input
        className="h-9 rounded-md bg-background text-foreground px-3 text-sm border border-input"
        placeholder={t('chat.search_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => {
          searchRef.current = e.detail.value
          setSearchQ(e.detail.value)
          void runSearch()
        }}
      />
      {searchQ && searchHits.length > 0 ? (
        <view className="rounded-md bg-card border border-border p-2 gap-1 max-h-[180px] overflow-auto">
          {searchHits.map((h, i) => (
            <view
              key={i}
              className="rounded-md bg-secondary p-2 gap-0.5"
              bindtap={() => {
                setActiveId(h.conversation_id)
                searchRef.current = ''
                setSearchQ('')
                setSearchHits([])
              }}
            >
              <text className="text-secondary-foreground text-sm font-medium">
                {h.title || '(untitled)'}
              </text>
              <text className="text-xs text-muted-foreground">{h.snippet}</text>
            </view>
          ))}
        </view>
      ) : null}

      {/* Project picker */}
      <view className="flex-row items-center gap-1 overflow-auto">
        {[{ id: 'all', name: t('chat.all_projects') }, ...projects].map((p) => (
          <view
            key={p.id}
            className={
              activeProject === p.id
                ? 'h-7 rounded-md bg-primary items-center justify-center px-2'
                : 'h-7 rounded-md bg-secondary items-center justify-center px-2'
            }
            bindtap={() => setActiveProject(p.id)}
          >
            <text
              className={
                activeProject === p.id
                  ? 'text-primary-foreground text-xs font-medium'
                  : 'text-secondary-foreground text-xs'
              }
            >
              {p.name}
            </text>
          </view>
        ))}
        <view
          className="h-7 rounded-md bg-secondary items-center justify-center px-2"
          bindtap={() => {
            const w = globalThis as { prompt?: (m: string) => string | null }
            const name = w.prompt?.(t('chat.project_name'))?.trim()
            if (name) {
              const p = addProject(name)
              setActiveProject(p.id)
            }
          }}
        >
          <text className="text-secondary-foreground text-xs">+</text>
        </view>
      </view>

      {/* Conversation list strip */}
      <view className="flex-row items-center gap-2 overflow-auto">
        <view
          className="h-8 rounded-md bg-primary items-center justify-center px-3"
          bindtap={() => {
            void (async () => {
              await newConversation()
              // If a project filter is active, attach the new chat to it.
              if (activeProject !== 'all') {
                // newConversation sets activeId to the newly created chat.
                const id = useProjects.getState().byConv
                void id // appease unused
                setTimeout(() => {
                  const cur = (globalThis as { __simuActiveConv?: string }).__simuActiveConv
                  if (cur) assignProj(cur, activeProject)
                }, 100)
              }
            })()
          }}
          aria-label={t('chat.new_conversation')}
        >
          <text className="text-primary-foreground text-sm font-medium">+ {t('chat.new')}</text>
        </view>
        {filteredConvs.map((c) => (
          <view
            key={c.id}
            className={
              activeId === c.id
                ? 'h-8 rounded-md bg-secondary border border-primary items-center px-2 flex-row gap-1'
                : 'h-8 rounded-md bg-secondary items-center px-2 flex-row gap-1'
            }
            aria-label={c.title || 'Untitled'}
          >
            <text
              className="text-secondary-foreground text-sm"
              bindtap={() => setActiveId(c.id)}
            >
              {c.title || c.id.slice(0, 8)}
            </text>
            {activeId === c.id ? (
              <>
                <text
                  className="text-muted-foreground text-xs px-1"
                  bindtap={() => void renameConversation(c.id)}
                  aria-label={t('chat.rename')}
                >
                  ✎
                </text>
                <text
                  className="text-destructive text-xs px-1"
                  bindtap={() => void deleteConversation(c.id)}
                  aria-label={t('chat.delete')}
                >
                  ✕
                </text>
              </>
            ) : null}
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
                m.content ? (
                  <LynxMarkdown source={m.content} />
                ) : (
                  <text className="text-foreground text-sm">{streaming ? '…' : ''}</text>
                )
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

      {/* Conversation settings + share + speak buttons */}
      {active ? (
        <view className="flex-row gap-2 px-1">
          <view
            className="h-7 rounded-md bg-secondary border border-border items-center justify-center px-3"
            bindtap={() => setSettingsOpen(true)}
            aria-label={t('chat.settings')}
          >
            <text className="text-secondary-foreground text-xs">⚙</text>
          </view>
          <view
            className="h-7 rounded-md bg-secondary border border-border items-center justify-center px-3"
            bindtap={() => void shareConversation()}
            aria-label={t('chat.share')}
          >
            <text className="text-secondary-foreground text-xs">
              🔗{active.shared ? ' ✓' : ''}
            </text>
          </view>
          <view
            className="h-7 rounded-md bg-secondary border border-border items-center justify-center px-3"
            bindtap={speakLast}
            aria-label={t('chat.speak')}
          >
            <text className="text-secondary-foreground text-xs">🔊</text>
          </view>
          {artifacts.length > 0 ? (
            <view
              className="h-7 rounded-md bg-secondary border border-border items-center justify-center px-3"
              bindtap={() => setArtifactsOpen(true)}
              aria-label={t('chat.artifacts')}
            >
              <text className="text-secondary-foreground text-xs">
                ⌗ {artifacts.length}
              </text>
            </view>
          ) : null}
        </view>
      ) : null}

      {/* Artifacts panel */}
      {artifactsOpen ? (
        <view
          className="fixed inset-0 bg-background/95 z-[9000] p-6"
          bindtap={() => setArtifactsOpen(false)}
        >
          <view className="rounded-md bg-card border border-border p-4 gap-3 max-h-[90%] overflow-auto">
            <text className="text-foreground text-sm font-medium">{t('chat.artifacts')}</text>
            {artifacts.map((a, i) => (
              <view key={i} className="rounded-md bg-background border border-border p-3 gap-2">
                <text className="text-xs text-muted-foreground">{a.lang}</text>
                <text className="text-foreground text-xs font-mono whitespace-pre-wrap">
                  {a.code.length > 1000 ? a.code.slice(0, 1000) + '\n…' : a.code}
                </text>
                <view
                  className="h-7 rounded-md bg-primary items-center justify-center"
                  bindtap={() => {
                    const w = globalThis as { navigator?: { clipboard?: { writeText: (s: string) => Promise<void> } } }
                    void w.navigator?.clipboard?.writeText(a.code)
                  }}
                >
                  <text className="text-primary-foreground text-xs font-medium">{t('chat.copy')}</text>
                </view>
              </view>
            ))}
          </view>
        </view>
      ) : null}

      {/* Settings dialog */}
      {settingsOpen && active ? (
        <view className="rounded-md bg-card border border-border p-3 gap-2">
          <text className="text-foreground text-sm font-medium">{t('chat.settings')}</text>
          <text className="text-xs text-muted-foreground">{t('chat.system_prompt')}</text>
          <input
            className="h-9 rounded-md bg-background text-foreground px-3 text-sm border border-input"
            placeholder={t('chat.system_prompt_placeholder')}
            type="text"
            bindinput={(e: { detail: { value: string } }) => {
              void updateSettings({ system_prompt: e.detail.value })
            }}
          />
          <text className="text-xs text-muted-foreground">
            {t('chat.temperature')}: {(active.temperature ?? 0.7).toFixed(2)}
          </text>
          <view className="flex-row gap-2">
            {[0.0, 0.3, 0.7, 1.0, 1.5].map((v) => (
              <view
                key={v}
                className={
                  (active.temperature ?? 0.7) === v
                    ? 'flex-1 h-8 rounded-md bg-primary items-center justify-center'
                    : 'flex-1 h-8 rounded-md bg-background border border-input items-center justify-center'
                }
                bindtap={() => void updateSettings({ temperature: v })}
              >
                <text
                  className={
                    (active.temperature ?? 0.7) === v
                      ? 'text-primary-foreground text-xs font-medium'
                      : 'text-foreground text-xs font-medium'
                  }
                >
                  {v.toFixed(1)}
                </text>
              </view>
            ))}
          </view>
          <view
            className="h-8 rounded-md bg-background border border-input items-center justify-center"
            bindtap={() => setSettingsOpen(false)}
          >
            <text className="text-foreground text-xs font-medium">{t('chat.close')}</text>
          </view>
        </view>
      ) : null}

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
          className="h-10 rounded-md bg-secondary border border-border items-center justify-center px-3"
          bindtap={attachFile}
          aria-label={t('chat.attach')}
        >
          <text className="text-secondary-foreground text-sm">📎</text>
        </view>
        <view
          className="h-10 rounded-md bg-secondary border border-border items-center justify-center px-3"
          bindtap={startVoice}
          aria-label={t('chat.voice')}
        >
          <text className="text-secondary-foreground text-sm">🎤</text>
        </view>
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

