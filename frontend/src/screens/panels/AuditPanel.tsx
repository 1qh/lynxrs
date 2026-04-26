import { useCallback, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'

export function AuditPanel() {
  const [audit, setAudit] = useState<Array<{ action: string; ip?: string | null; created_at: string }>>([])
  const load = useCallback(async () => {
    const { data } = await api.GET('/me/audit', {})
    if (data) setAudit(data as Array<{ action: string; ip?: string | null; created_at: string }>)
  }, [])
  return (
    <view>
      <view className="Button ButtonGhost" bindtap={load}>
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
    </view>
  )
}
