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
  const [mfaSecret, setMfaSecret] = useState<string | null>(null)
  const [audit, setAudit] = useState<Array<{ action: string; ip?: string | null; created_at: string }>>([])
  const [webhooks, setWebhooks] = useState<Array<{ id: string; url: string; enabled: boolean }>>([])
  const [webhookUrl, setWebhookUrl] = useState('')
  const [webhookSecret, setWebhookSecret] = useState<string | null>(null)
  const [trash, setTrash] = useState<FileDto[]>([])
  const [orgs, setOrgs] = useState<Array<{ id: string; name: string; slug: string }>>([])
  const [displayName, setDisplayName] = useState<string>(user.display_name ?? '')
  const [starred, setStarred] = useState<FileDto[]>([])

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
        if (msg.kind === 'file_created' || msg.kind === 'file_deleted') void refresh()
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

  const refreshStarred = useCallback(async () => {
    const { data } = await api.GET('/files/starred', {})
    if (data) setStarred(data as FileDto[])
  }, [])

  const toggleStar = useCallback(async (id: string) => {
    await api.POST('/files/{id}/star', { params: { path: { id } } })
    void refreshStarred()
  }, [refreshStarred])

  const saveProfile = useCallback(async () => {
    const name = displayName.trim()
    if (!name) return
    const { data } = await api.PATCH('/auth/me', { body: { display_name: name } })
    if (data) setUser(data as typeof user)
  }, [displayName, setUser])

  const refreshOrgs = useCallback(async () => {
    const { data } = await api.GET('/orgs', {})
    if (data) setOrgs(data as Array<{ id: string; name: string; slug: string }>)
  }, [])

  const [orgName, setOrgName] = useState('')
  const [orgSlug, setOrgSlug] = useState('')

  const createOrg = useCallback(async () => {
    const name = orgName.trim()
    const slug = orgSlug.trim()
    if (!name || !slug) return
    await api.POST('/orgs', { body: { name, slug } })
    setOrgName('')
    setOrgSlug('')
    void refreshOrgs()
  }, [orgName, orgSlug, refreshOrgs])

  const refreshTrash = useCallback(async () => {
    const { data } = await api.GET('/trash', {})
    if (data) setTrash(((data as unknown) as { items: FileDto[] }).items ?? [])
  }, [])

  const restoreFile = useCallback(async (id: string) => {
    await api.POST('/trash/{id}/restore', { params: { path: { id } } })
    void refresh(); void refreshTrash()
  }, [refresh])

  const purgeFile = useCallback(async (id: string) => {
    await api.DELETE('/trash/{id}', { params: { path: { id } } })
    void refreshTrash()
  }, [])

  const refreshWebhooks = useCallback(async () => {
    const { data } = await api.GET('/webhooks', {})
    if (data) setWebhooks(data as Array<{ id: string; url: string; enabled: boolean }>)
  }, [])

  const createWebhook = useCallback(async () => {
    const url = webhookUrl.trim()
    if (!url) return
    const { data } = await api.POST('/webhooks', { body: { url } })
    if (data) {
      const d = data as { secret: string }
      setWebhookSecret(d.secret)
      setWebhookUrl('')
      void refreshWebhooks()
    }
  }, [webhookUrl, refreshWebhooks])

  const revokeWebhook = useCallback(async (id: string) => {
    await api.DELETE('/webhooks/{id}', { params: { path: { id } } })
    void refreshWebhooks()
  }, [refreshWebhooks])

  const loadAudit = useCallback(async () => {
    const { data } = await api.GET('/me/audit', {})
    if (data) setAudit(data as Array<{ action: string; ip?: string | null; created_at: string }>)
  }, [])

  const mfaEnroll = useCallback(async () => {
    const { data, error } = await api.POST('/mfa/enroll', {})
    if (error) { console.error('[mfa]', error); return }
    setMfaSecret((data as { secret: string }).secret)
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
            <view key={f.id} className="FileRow">
              <text className="FileName" bindtap={() => void share(f.id)}>{f.filename}</text>
              <text className="FileMeta">{f.size_bytes}B · {f.content_type}</text>
              <view className="Button ButtonGhost" bindtap={() => void toggleStar(f.id)}>
                <text className="ButtonText">⭐</text>
              </view>
            </view>
          ))
        )}
      </view>
      {shareUrl ? (
        <text className="Muted">share: {shareUrl}</text>
      ) : null}
      <text className="Muted">display_name: {user.display_name ?? '—'}</text>
      <input
        className="Input"
        placeholder="your display name"
        type="text"
        value={displayName}
        bindinput={(e: { detail: { value: string } }) => setDisplayName(e.detail.value)}
      />
      <view className="Button ButtonGhost" bindtap={saveProfile}>
        <text className="ButtonText">Save profile</text>
      </view>
      <view className="Button ButtonGhost" bindtap={refreshStarred}>
        <text className="ButtonText">Load starred ({starred.length})</text>
      </view>
      <view className="Button ButtonGhost" bindtap={refreshOrgs}>
        <text className="ButtonText">Load orgs ({orgs.length})</text>
      </view>
      <input
        className="Input"
        placeholder="Org name"
        type="text"
        value={orgName}
        bindinput={(e: { detail: { value: string } }) => setOrgName(e.detail.value)}
      />
      <input
        className="Input"
        placeholder="slug (a-z0-9-)"
        type="text"
        value={orgSlug}
        bindinput={(e: { detail: { value: string } }) => setOrgSlug(e.detail.value)}
      />
      <view className="Button ButtonGhost" bindtap={createOrg}>
        <text className="ButtonText">Create org</text>
      </view>
      {orgs.length > 0 ? (
        <view className="OrgList">
          {orgs.map((o) => (
            <view key={o.id} className="OrgRow">
              <text className="FileName">{o.name}</text>
              <text className="Muted"> · {o.slug}</text>
            </view>
          ))}
        </view>
      ) : null}
      <view className="Button ButtonGhost" bindtap={refreshTrash}>
        <text className="ButtonText">Load trash</text>
      </view>
      {trash.length > 0 ? (
        <view className="TrashList">
          {trash.map((f) => (
            <view key={f.id} className="TrashRow">
              <text className="FileName">{f.filename}</text>
              <view className="Button ButtonGhost" bindtap={() => void restoreFile(f.id)}>
                <text className="ButtonText">restore</text>
              </view>
              <view className="Button ButtonGhost" bindtap={() => void purgeFile(f.id)}>
                <text className="ButtonText">purge</text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
      <view className="Button ButtonGhost" bindtap={refreshWebhooks}>
        <text className="ButtonText">Load webhooks</text>
      </view>
      {webhooks.length > 0 ? (
        <view className="WebhookList">
          {webhooks.map((w) => (
            <view key={w.id} className="WebhookRow">
              <text className="WebhookUrl">{w.url}</text>
              <view className="Button ButtonGhost" bindtap={() => void revokeWebhook(w.id)}>
                <text className="ButtonText">revoke</text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
      <input
        className="Input"
        placeholder="https://your-host/hook"
        type="url"
        value={webhookUrl}
        bindinput={(e: { detail: { value: string } }) => setWebhookUrl(e.detail.value)}
      />
      <view className="Button" bindtap={createWebhook}>
        <text className="ButtonText">Register webhook</text>
      </view>
      {webhookSecret ? (
        <text className="Muted">webhook secret (copy now, shown once): {webhookSecret}</text>
      ) : null}
      <view className="Button ButtonGhost" bindtap={loadAudit}>
        <text className="ButtonText">Load audit log</text>
      </view>
      {audit.length > 0 ? (
        <view className="AuditList">
          {audit.slice(0, 20).map((a, i) => (
            <view key={i} className="AuditRow">
              <text className="AuditAction">{a.action}</text>
              <text className="Muted"> · {a.ip ?? 'n/a'} · {a.created_at.slice(0, 19)}</text>
            </view>
          ))}
        </view>
      ) : null}
      {!user.totp_enabled ? (
        <view className="Button ButtonGhost" bindtap={mfaEnroll}>
          <text className="ButtonText">Enable MFA (TOTP)</text>
        </view>
      ) : (
        <text className="Muted">MFA enabled ✓</text>
      )}
      {mfaSecret ? (
        <text className="Muted">MFA secret: {mfaSecret} — scan in authenticator, then POST /api/mfa/activate</text>
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
