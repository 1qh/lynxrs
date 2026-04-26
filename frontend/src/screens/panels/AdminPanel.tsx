import { useCallback, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'
import type { components } from '../../api/schema.js'

type AdminStats = components['schemas']['AdminStats']

export function AdminPanel() {
  const [stats, setStats] = useState<AdminStats | null>(null)
  const load = useCallback(async () => {
    const { data } = await api.GET('/admin/stats', {})
    if (data) setStats(data as AdminStats)
  }, [])
  return (
    <view className="AdminPanel">
      <view className="Button ButtonGhost" bindtap={load}>
        <text className="ButtonText">Admin stats</text>
      </view>
      {stats ? (
        <text className="Muted">
          users: {stats.users} · files: {stats.files} · bytes: {stats.total_bytes}
        </text>
      ) : null}
    </view>
  )
}
