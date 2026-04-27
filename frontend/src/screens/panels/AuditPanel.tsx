import { useCallback, useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'

type AuditRow = { action: string; ip?: string | null; created_at: string }

function relTime(iso: string): string {
  const t = new Date(iso).getTime()
  const dt = Math.max(0, Date.now() - t)
  const m = Math.floor(dt / 60_000)
  if (m < 1) return 'just now'
  if (m < 60) return `${m}m ago`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.floor(h / 24)
  if (d < 30) return `${d}d ago`
  return new Date(iso).toISOString().slice(0, 10)
}

function actionDot(action: string): string {
  if (action.includes('login')) return '🔐'
  if (action.includes('signup')) return '✨'
  if (action.includes('logout')) return '🚪'
  if (action.includes('file_upload') || action.includes('file_create')) return '📤'
  if (action.includes('file_delete') || action.includes('purge')) return '🗑'
  if (action.includes('share')) return '🔗'
  if (action.includes('mfa') || action.includes('totp')) return '🛡'
  if (action.includes('webhook')) return '📡'
  return '•'
}

export function AuditPanel() {
  const { t } = useTranslation()
  const [audit, setAudit] = useState<AuditRow[]>([])
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const { data } = await api.GET('/me/audit', {})
      if (data) setAudit(data as AuditRow[])
    } finally {
      setLoading(false)
    }
  }, [])

  // Auto-load on mount — activity feed should be ready, not click-to-fetch.
  useEffect(() => { void load() }, [load])

  return (
    <view className="gap-2">
      <view
        className="h-10 rounded-md bg-background border border-input items-center justify-center"
        bindtap={load}
      >
        <text className="text-foreground text-sm font-medium">
          {loading ? t('files.loading') : t('audit.load')}
        </text>
      </view>
      {audit.length === 0 && !loading ? (
        <view className="rounded-md border border-dashed border-border p-6 items-center">
          <text className="text-sm text-muted-foreground">{t('audit.empty')}</text>
        </view>
      ) : null}
      {audit.length > 0 ? (
        <view className="gap-0">
          {audit.slice(0, 50).map((a, i) => (
            <view
              key={i}
              className="flex-row gap-3 py-2 border-b border-border items-start"
            >
              <text className="text-base w-6 text-center">{actionDot(a.action)}</text>
              <view className="flex-1 gap-0.5">
                <text className="text-foreground text-sm font-medium">{a.action}</text>
                <text className="text-xs text-muted-foreground">
                  {relTime(a.created_at)} · {a.ip ?? 'n/a'}
                </text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
    </view>
  )
}
