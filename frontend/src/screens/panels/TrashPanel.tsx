import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import type { components } from '../../api/schema.js'

type FileDto = components['schemas']['FileDto']

export function TrashPanel({ onRestore }: { onRestore?: () => void }) {
  const { t } = useTranslation()
  const [trash, setTrash] = useState<FileDto[]>([])

  const refresh = useCallback(async () => {
    const { data } = await api.GET('/trash', {})
    if (data) setTrash(((data as unknown) as { items: FileDto[] }).items ?? [])
  }, [])

  const restore = useCallback(async (id: string) => {
    await api.POST('/trash/{id}/restore', { params: { path: { id } } })
    void refresh()
    onRestore?.()
  }, [refresh, onRestore])

  const purge = useCallback(async (id: string) => {
    await api.DELETE('/trash/{id}', { params: { path: { id } } })
    void refresh()
  }, [refresh])

  return (
    <view className="gap-2">
      <view
        className="h-10 rounded-md bg-background border border-input items-center justify-center"
        bindtap={refresh}
      >
        <text className="text-foreground text-sm font-medium">{t('trash.load')}</text>
      </view>
      {trash.length > 0 ? (
        <view className="gap-2">
          {trash.map((f) => (
            <view key={f.id} className="rounded-md bg-card border border-border p-3 gap-2">
              <text className="text-foreground text-sm font-medium">{f.filename}</text>
              <view className="flex-row gap-2">
                <view
                  className="h-9 flex-1 rounded-md bg-background border border-input items-center justify-center"
                  bindtap={() => void restore(f.id)}
                >
                  <text className="text-foreground text-sm font-medium">
                    {t('trash.restore')}
                  </text>
                </view>
                <view
                  className="h-9 flex-1 rounded-md bg-destructive items-center justify-center"
                  bindtap={() => void purge(f.id)}
                >
                  <text className="text-destructive-foreground text-sm font-medium">
                    {t('trash.purge')}
                  </text>
                </view>
              </view>
            </view>
          ))}
        </view>
      ) : null}
    </view>
  )
}
