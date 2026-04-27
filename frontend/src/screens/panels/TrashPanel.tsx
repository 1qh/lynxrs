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
    <view>
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={refresh}
      >
        <text className="text-white text-base font-semibold">{t('trash.load')}</text>
      </view>
      {trash.length > 0 ? (
        <view className="mt-3 gap-2">
          {trash.map((f) => (
            <view key={f.id} className="bg-card rounded-[10px] p-3 gap-1">
              <text className="text-white text-[15px] font-medium">{f.filename}</text>
              <view
                className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
                bindtap={() => void restore(f.id)}
              >
                <text className="text-white text-base font-semibold">{t('trash.restore')}</text>
              </view>
              <view
                className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
                bindtap={() => void purge(f.id)}
              >
                <text className="text-white text-base font-semibold">{t('trash.purge')}</text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
    </view>
  )
}
