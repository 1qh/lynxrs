import { useCallback, useEffect, useRef, useState } from '@lynx-js/react'
import { api } from './api/client.js'
import { useAuth, type User } from './state/auth.js'
import type { components } from './api/schema.js'
import './App.css'

type FileDto = components['schemas']['FileDto']

export function App() {
  const user = useAuth((s) => s.user)
  const setUser = useAuth((s) => s.setUser)
  const [boot, setBoot] = useState(true)

  useEffect(() => {
    ;(async () => {
      const { data } = await api.GET('/auth/me', {})
      if (data) setUser(data as User)
      setBoot(false)
    })()
  }, [setUser])

  if (boot) {
    return (
      <view className="Screen">
        <text className="Subtitle">loading…</text>
      </view>
    )
  }

  return (
    <view className="Screen">
      <view className="Frame">
        <text className="Title">simu</text>
        {user ? <Home /> : <AuthForm />}
      </view>
    </view>
  )
}

function AuthForm() {
  const setUser = useAuth((s) => s.setUser)
  const [mode, setMode] = useState<'login' | 'signup'>('signup')
  const emailRef = useRef('demo@simu.dev')
  const passwordRef = useRef('hunter2hunter2')
  const [err, setErr] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = useCallback(async () => {
    setErr(null); setBusy(true)
    try {
      const body = { email: emailRef.current, password: passwordRef.current }
      if (mode === 'signup') {
        const { data, error } = await api.POST('/auth/signup', { body })
        if (error) setErr((error as { message?: string }).message ?? 'failed')
        else if (data) setUser(data as User)
      } else {
        const { data, error } = await api.POST('/auth/login', { body })
        if (error) setErr((error as { message?: string }).message ?? 'failed')
        else if (data) setUser(data as User)
      }
    } catch (e) {
      setErr(String(e))
    } finally {
      setBusy(false)
    }
  }, [mode, setUser])

  return (
    <view className="Card">
      <text className="H2">{mode === 'signup' ? 'Create account' : 'Login'}</text>
      <input
        className="Input"
        placeholder="email"
        type="email"
        bindinput={(e: { detail: { value: string } }) => { emailRef.current = e.detail.value }}
      />
      <input
        className="Input"
        placeholder="password"
        type="password"
        bindinput={(e: { detail: { value: string } }) => { passwordRef.current = e.detail.value }}
      />
      {err ? <text className="Error">{err}</text> : null}
      <view className="Button" bindtap={busy ? undefined : submit}>
        <text className="ButtonText">{busy ? '…' : mode === 'signup' ? 'Sign up' : 'Log in'}</text>
      </view>
      <view
        className="SwitchRow"
        bindtap={() => setMode(mode === 'signup' ? 'login' : 'signup')}
      >
        <text className="SwitchText">
          {mode === 'signup' ? 'Have an account? Log in' : 'New here? Sign up'}
        </text>
      </view>
    </view>
  )
}

type AdminStats = components['schemas']['AdminStats']

function Home() {
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [files, setFiles] = useState<FileDto[]>([])
  const [busy, setBusy] = useState(false)
  const [stats, setStats] = useState<AdminStats | null>(null)
  const [shareUrl, setShareUrl] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    const { data } = await api.GET('/files', { params: { query: {} } })
    if (data) setFiles((data as { items: FileDto[] }).items ?? [])
  }, [])

  useEffect(() => { void refresh() }, [refresh])

  // Live updates via WebSocket: refresh file list on server broadcast.
  useEffect(() => {
    const base = (import.meta.env?.PUBLIC_API_BASE as string | undefined) ?? 'http://localhost:8088'
    const wsUrl = base.replace(/^http/, 'ws') + '/events/ws'
    let ws: WebSocket | null = null
    try { ws = new WebSocket(wsUrl) } catch { return }
    if (!ws) return
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(String(ev.data)) as { kind?: string }
        if (msg.kind === 'file_created') void refresh()
      } catch {}
    }
    return () => { try { ws?.close() } catch {} }
  }, [refresh])

  const logout = useCallback(async () => {
    await api.POST('/auth/logout', {})
    setUser(null)
  }, [setUser])

  const logoutAll = useCallback(async () => {
    await api.POST('/auth/logout-all', {})
    setUser(null)
  }, [setUser])

  const upload = useCallback(async () => {
    setBusy(true)
    try {
      const content = `hello from lynx ${new Date().toISOString()}`
      // Lynx has no FormData/Blob APIs — use JSON base64.
      const data_base64 = btoa(unescape(encodeURIComponent(content)))
      const { error } = await api.POST('/files/json', {
        body: {
          filename: `note-${Date.now()}.txt`,
          content_type: 'text/plain',
          data_base64,
        },
      })
      if (error) console.error('[upload] error', error)
      await refresh()
    } catch (e) {
      console.error('[upload] threw', String(e))
    } finally {
      setBusy(false)
    }
  }, [refresh])

  const resend = useCallback(async () => {
    await api.POST('/auth/email/resend', {})
  }, [])

  const share = useCallback(async (id: string) => {
    const { data, error } = await api.POST('/files/{id}/shares', {
      params: { path: { id } },
      body: { ttl_hours: 24 },
    })
    if (error) { console.error('[share]', error); return }
    setShareUrl((data as { url: string }).url)
  }, [])

  const loadStats = useCallback(async () => {
    const { data } = await api.GET('/admin/stats', {})
    if (data) setStats(data as AdminStats)
  }, [])

  return (
    <view className="Card">
      <text className="H2">Hello {user.email}</text>
      {!user.email_verified ? (
        <view className="Banner" bindtap={resend}>
          <text className="BannerText">
            Email not verified · tap to resend
          </text>
        </view>
      ) : null}
      {user.role === 'admin' ? (
        <view className="AdminPanel">
          <view className="Button ButtonGhost" bindtap={loadStats}>
            <text className="ButtonText">Admin stats</text>
          </view>
          {stats ? (
            <text className="Muted">
              users: {stats.users} · files: {stats.files} · bytes: {stats.total_bytes}
            </text>
          ) : null}
        </view>
      ) : null}
      <view className="Button" bindtap={busy ? undefined : upload}>
        <text className="ButtonText">{busy ? 'uploading…' : 'Upload sample file'}</text>
      </view>
      <view className="FileList">
        {files.length === 0 ? (
          <text className="Muted">no files yet</text>
        ) : (
          files.map((f) => (
            <view key={f.id} className="FileRow" bindtap={() => void share(f.id)}>
              <text className="FileName">{f.filename}</text>
              <text className="FileMeta">{f.size_bytes}B · {f.content_type} · tap to share</text>
            </view>
          ))
        )}
      </view>
      {shareUrl ? (
        <text className="Muted">share: {shareUrl}</text>
      ) : null}
      <view className="Button ButtonGhost" bindtap={logout}>
        <text className="ButtonText">Log out</text>
      </view>
      <view className="Button ButtonGhost" bindtap={logoutAll}>
        <text className="ButtonText">Log out all devices</text>
      </view>
    </view>
  )
}
