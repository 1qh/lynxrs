import { useCallback, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'
import type { components } from '../../api/schema.js'

type FileDto = components['schemas']['FileDto']

export function TrashPanel({ onRestore }: { onRestore?: () => void }) {
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
      <view className="Button ButtonGhost" bindtap={refresh}>
        <text className="ButtonText">Load trash</text>
      </view>
      {trash.length > 0 ? (
        <view className="TrashList">
          {trash.map((f) => (
            <view key={f.id} className="TrashRow">
              <text className="FileName">{f.filename}</text>
              <view className="Button ButtonGhost" bindtap={() => void restore(f.id)}>
                <text className="ButtonText">restore</text>
              </view>
              <view className="Button ButtonGhost" bindtap={() => void purge(f.id)}>
                <text className="ButtonText">purge</text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
    </view>
  )
}
