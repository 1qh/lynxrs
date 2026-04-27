import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'

export function OrgsPanel() {
  const { t } = useTranslation()
  const [orgs, setOrgs] = useState<Array<{ id: string; name: string; slug: string }>>([])
  const [orgName, setOrgName] = useState('')
  const [orgSlug, setOrgSlug] = useState('')
  const [orgDetail, setOrgDetail] = useState<null | {
    id: string
    stats?: { members: number; files: number; total_bytes: number }
    members?: Array<{ email: string; role: string }>
  }>(null)

  const refresh = useCallback(async () => {
    const { data } = await api.GET('/orgs', {})
    if (data) setOrgs(data as Array<{ id: string; name: string; slug: string }>)
  }, [])

  const create = useCallback(async () => {
    const name = orgName.trim()
    const slug = orgSlug.trim()
    if (!name || !slug) return
    await api.POST('/orgs', { body: { name, slug } })
    setOrgName('')
    setOrgSlug('')
    void refresh()
  }, [orgName, orgSlug, refresh])

  const openDetail = useCallback(async (id: string) => {
    const [stats, members] = await Promise.all([
      api.GET('/orgs/{id}/stats', { params: { path: { id } } }),
      api.GET('/orgs/{id}/members', { params: { path: { id } } }),
    ])
    setOrgDetail({
      id,
      stats: (stats.data as { members: number; files: number; total_bytes: number }) ?? undefined,
      members: (members.data as Array<{ email: string; role: string }>) ?? [],
    })
  }, [])

  return (
    <view>
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={refresh}
      >
        <text className="text-white text-base font-semibold">
          {t('orgs.load', { count: orgs.length })}
        </text>
      </view>
      <input
        className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
        placeholder={t('orgs.name_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setOrgName(e.detail.value)}
      />
      <input
        className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
        placeholder={t('orgs.slug_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setOrgSlug(e.detail.value)}
      />
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={create}
      >
        <text className="text-white text-base font-semibold">{t('orgs.create')}</text>
      </view>
      {orgs.length > 0 ? (
        <view className="mt-3 gap-2">
          {orgs.map((o) => (
            <view
              key={o.id}
              className="bg-card rounded-[10px] p-3 gap-1"
              bindtap={() => void openDetail(o.id)}
            >
              <text className="text-white text-[15px] font-medium">{o.name}</text>
              <text className="text-muted text-sm py-2.5"> · {o.slug}</text>
            </view>
          ))}
        </view>
      ) : null}
      {orgDetail ? (
        <view className="mt-3">
          <text className="text-muted text-sm py-2.5">
            {orgDetail.stats
              ? t('orgs.detail_summary', {
                  id: orgDetail.id.slice(0, 8),
                  members: orgDetail.stats.members,
                  files: orgDetail.stats.files,
                  bytes: orgDetail.stats.total_bytes,
                })
              : `${orgDetail.id.slice(0, 8)}…`}
          </text>
          {(orgDetail.members ?? []).map((m, i) => (
            <text key={i} className="text-muted text-sm py-2.5">
              · {m.email} ({m.role})
            </text>
          ))}
        </view>
      ) : null}
    </view>
  )
}
