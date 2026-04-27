import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'

export function AuditPanel() {
  const { t } = useTranslation()
  const [audit, setAudit] = useState<Array<{ action: string; ip?: string | null; created_at: string }>>([])
  const load = useCallback(async () => {
    const { data } = await api.GET('/me/audit', {})
    if (data) setAudit(data as Array<{ action: string; ip?: string | null; created_at: string }>)
  }, [])
  return (
    <view>
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={load}
      >
        <text className="text-white text-base font-semibold">{t('audit.load')}</text>
      </view>
      {audit.length > 0 ? (
        <view>
          {audit.slice(0, 20).map((a, i) => (
            <view key={i}>
              <text className="text-white text-[15px] font-medium">{a.action}</text>
              <text className="text-muted text-sm py-2.5">
                · {a.ip ?? 'n/a'} · {a.created_at.slice(0, 19)}
              </text>
            </view>
          ))}
        </view>
      ) : null}
    </view>
  )
}
