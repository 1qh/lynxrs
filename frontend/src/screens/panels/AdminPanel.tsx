import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import type { components } from '../../api/schema.js'

type AdminStats = components['schemas']['AdminStats']

export function AdminPanel() {
  const { t } = useTranslation()
  const [stats, setStats] = useState<AdminStats | null>(null)
  const load = useCallback(async () => {
    const { data } = await api.GET('/admin/stats', {})
    if (data) setStats(data as AdminStats)
  }, [])
  return (
    <view className="gap-1.5 py-2.5 border-t border-border mt-2">
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={load}
      >
        <text className="text-white text-base font-semibold">{t('admin.stats')}</text>
      </view>
      {stats ? (
        <text className="text-muted text-sm py-2.5">
          {t('admin.stats_summary', {
            users: stats.users,
            files: stats.files,
            bytes: stats.total_bytes,
          })}
        </text>
      ) : null}
    </view>
  )
}
