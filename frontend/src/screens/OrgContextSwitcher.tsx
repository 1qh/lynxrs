import { useCallback, useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../api/client.js'
import { useOrgContext } from '../state/orgContext.js'

type Org = { id: string; name: string; slug: string }

export function OrgContextSwitcher() {
  const { t } = useTranslation()
  const activeOrgId = useOrgContext((s) => s.activeOrgId)
  const setActive = useOrgContext((s) => s.setActive)
  const [orgs, setOrgs] = useState<Org[]>([])
  const [open, setOpen] = useState(false)

  useEffect(() => {
    void (async () => {
      const { data } = await api.GET('/orgs', {})
      if (data) setOrgs(data as Org[])
    })()
  }, [])

  const close = useCallback(() => setOpen(false), [])
  const active = orgs.find((o) => o.id === activeOrgId)
  const label = active ? active.name : t('org.personal')

  return (
    <view className="relative">
      <view
        className="h-9 rounded-md bg-secondary border border-border items-center justify-center px-3 flex-row gap-1"
        bindtap={() => setOpen((v) => !v)}
        aria-label={t('org.context_switcher')}
      >
        <text className="text-secondary-foreground text-sm font-medium">{label}</text>
        <text className="text-secondary-foreground text-xs">▾</text>
      </view>
      {open ? (
        <view className="fixed inset-0 z-[8000]" bindtap={close}>
          <view className="absolute right-5 top-20 rounded-md bg-popover border border-border p-2 gap-1 min-w-[200px]">
            <view
              className={
                activeOrgId === null
                  ? 'h-8 rounded-md bg-accent items-center justify-center px-3'
                  : 'h-8 rounded-md bg-transparent items-center justify-center px-3'
              }
              bindtap={() => { setActive(null); close() }}
              aria-label={t('org.personal')}
            >
              <text
                className={
                  activeOrgId === null
                    ? 'text-accent-foreground text-sm font-medium'
                    : 'text-popover-foreground text-sm'
                }
              >
                {t('org.personal')}
              </text>
            </view>
            {orgs.map((o) => (
              <view
                key={o.id}
                className={
                  activeOrgId === o.id
                    ? 'h-8 rounded-md bg-accent items-center justify-center px-3'
                    : 'h-8 rounded-md bg-transparent items-center justify-center px-3'
                }
                bindtap={() => { setActive(o.id); close() }}
                aria-label={o.name}
              >
                <text
                  className={
                    activeOrgId === o.id
                      ? 'text-accent-foreground text-sm font-medium'
                      : 'text-popover-foreground text-sm'
                  }
                >
                  {o.name}
                </text>
              </view>
            ))}
          </view>
        </view>
      ) : null}
    </view>
  )
}
