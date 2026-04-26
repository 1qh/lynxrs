import { useCallback, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'

export function OrgsPanel() {
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
      <view className="Button ButtonGhost" bindtap={refresh}>
        <text className="ButtonText">Load orgs ({orgs.length})</text>
      </view>
      <input
        className="Input"
        placeholder="Org name"
        type="text"
        bindinput={(e: { detail: { value: string } }) => setOrgName(e.detail.value)}
      />
      <input
        className="Input"
        placeholder="slug (a-z0-9-)"
        type="text"
        bindinput={(e: { detail: { value: string } }) => setOrgSlug(e.detail.value)}
      />
      <view className="Button ButtonGhost" bindtap={create}>
        <text className="ButtonText">Create org</text>
      </view>
      {orgs.length > 0 ? (
        <view className="OrgList">
          {orgs.map((o) => (
            <view key={o.id} className="OrgRow" bindtap={() => void openDetail(o.id)}>
              <text className="FileName">{o.name}</text>
              <text className="Muted"> · {o.slug}</text>
            </view>
          ))}
        </view>
      ) : null}
      {orgDetail ? (
        <view className="OrgDetail">
          <text className="Muted">
            Org {orgDetail.id.slice(0, 8)}…
            {orgDetail.stats
              ? ` — members: ${orgDetail.stats.members}, files: ${orgDetail.stats.files}, bytes: ${orgDetail.stats.total_bytes}`
              : ''}
          </text>
          {(orgDetail.members ?? []).map((m, i) => (
            <text key={i} className="Muted">· {m.email} ({m.role})</text>
          ))}
        </view>
      ) : null}
    </view>
  )
}
