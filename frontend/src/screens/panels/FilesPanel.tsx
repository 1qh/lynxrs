import { useCallback, useEffect, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'
import type { components } from '../../api/schema.js'

type FileDto = components['schemas']['FileDto']

export function FilesPanel({ refreshKey }: { refreshKey: number }) {
  const [files, setFiles] = useState<FileDto[]>([])
  const [busy, setBusy] = useState(false)
  const [shareUrl, setShareUrl] = useState<string | null>(null)
  const [starred, setStarred] = useState<FileDto[]>([])
  const [descEdit, setDescEdit] = useState<{ id: string; text: string } | null>(null)

  const refresh = useCallback(async () => {
    const { data } = await api.GET('/files', { params: { query: {} } })
    if (data) setFiles((data as { items: FileDto[] }).items ?? [])
  }, [])

  useEffect(() => { void refresh() }, [refresh, refreshKey])

  const upload = useCallback(async () => {
    setBusy(true)
    try {
      const content = `hello from lynx ${new Date().toISOString()}`
      const data_base64 = btoa(unescape(encodeURIComponent(content)))
      const { error } = await api.POST('/files/json', {
        body: {
          filename: `note-${Date.now()}.txt`,
          content_type: 'text/plain',
          data_base64,
        },
      })
      if (error) console.error('[upload] error', error)
      await refresh()
    } catch (e) {
      console.error('[upload] threw', String(e))
    } finally {
      setBusy(false)
    }
  }, [refresh])

  const share = useCallback(async (id: string) => {
    const { data, error } = await api.POST('/files/{id}/shares', {
      params: { path: { id } },
      body: { ttl_hours: 24 },
    })
    if (error) { console.error('[share]', error); return }
    setShareUrl((data as { url: string }).url)
  }, [])

  const saveDescribe = useCallback(async () => {
    if (!descEdit) return
    await api.PATCH('/files/{id}/describe', {
      params: { path: { id: descEdit.id } },
      body: { description: descEdit.text },
    })
    setDescEdit(null)
    void refresh()
  }, [descEdit, refresh])

  const refreshStarred = useCallback(async () => {
    const { data } = await api.GET('/files/starred', {})
    if (data) setStarred(data as FileDto[])
  }, [])

  const toggleStar = useCallback(async (id: string) => {
    await api.POST('/files/{id}/star', { params: { path: { id } } })
    void refreshStarred()
  }, [refreshStarred])

  return (
    <view>
      <view className="Button" bindtap={busy ? undefined : upload}>
        <text className="ButtonText">{busy ? 'uploading…' : 'Upload sample file'}</text>
      </view>
      <view className="FileList">
        {files.length === 0 ? (
          <text className="Muted">no files yet</text>
        ) : (
          files.map((f) => (
            <view key={f.id} className="FileRow">
              <text className="FileName" bindtap={() => void share(f.id)}>{f.filename}</text>
              <text className="FileMeta">{f.size_bytes}B · {f.content_type}</text>
              {f.description ? <text className="Muted">{f.description}</text> : null}
              <view className="Button ButtonGhost" bindtap={() => void toggleStar(f.id)}>
                <text className="ButtonText">⭐</text>
              </view>
              <view className="Button ButtonGhost" bindtap={() => setDescEdit({ id: f.id, text: f.description ?? '' })}>
                <text className="ButtonText">✎ describe</text>
              </view>
            </view>
          ))
        )}
      </view>
      {shareUrl ? <text className="Muted">share: {shareUrl}</text> : null}
      <view className="Button ButtonGhost" bindtap={refreshStarred}>
        <text className="ButtonText">Load starred ({starred.length})</text>
      </view>
      {descEdit ? (
        <view className="DescEdit">
          <input
            className="Input"
            placeholder="file description"
            type="text"
            bindinput={(e: { detail: { value: string } }) =>
              setDescEdit({ id: descEdit.id, text: e.detail.value })
            }
          />
          <view className="Button" bindtap={saveDescribe}>
            <text className="ButtonText">Save description</text>
          </view>
        </view>
      ) : null}
    </view>
  )
}
