import { useCallback, useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import { reportError } from '../../state/toast.js'
import type { components } from '../../api/schema.js'

type FileDto = components['schemas']['FileDto']

export function FilesPanel({ refreshKey }: { refreshKey: number }) {
  const { t } = useTranslation()
  const [files, setFiles] = useState<FileDto[]>([])
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [shareUrl, setShareUrl] = useState<string | null>(null)
  const [starred, setStarred] = useState<FileDto[]>([])
  const [descEdit, setDescEdit] = useState<{ id: string; text: string } | null>(null)
  const [query, setQuery] = useState('')

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const { data, error } = await api.GET('/files', { params: { query: {} } })
      if (error) reportError(error, 'Load files failed')
      if (data) setFiles((data as { items: FileDto[] }).items ?? [])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { void refresh() }, [refresh, refreshKey])

  const uploadSample = useCallback(async () => {
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
      if (error) reportError(error, 'Upload failed')
      await refresh()
    } catch (e) {
      reportError(e, 'Upload threw')
    } finally {
      setBusy(false)
    }
  }, [refresh])

  const uploadBlob = useCallback(
    async (f: File) => {
      setBusy(true)
      try {
        const ab = await f.arrayBuffer()
        const bytes = new Uint8Array(ab)
        let bin = ''
        for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!)
        const data_base64 = btoa(bin)
        const { error } = await api.POST('/files/json', {
          body: {
            filename: f.name,
            content_type: f.type || 'application/octet-stream',
            data_base64,
          },
        })
        if (error) reportError(error, 'Upload failed')
        await refresh()
      } catch (e) {
        reportError(e, 'Upload threw')
      } finally {
        setBusy(false)
      }
    },
    [refresh],
  )

  // Real-file picker — web-only. On platforms without `<input type=file>`
  // (Lynx native), the click is a no-op; the sample button stays as fallback.
  const pickAndUpload = useCallback(() => {
    const doc = (globalThis as { document?: Document }).document
    if (!doc) return
    const el = doc.createElement('input')
    el.type = 'file'
    el.onchange = () => {
      const f = el.files?.[0]
      if (f) void uploadBlob(f)
    }
    el.click()
  }, [uploadBlob])

  // Drag-drop: register on document so drops anywhere on the page upload.
  // Lynx's <view> doesn't bubble HTML5 drag events naturally, so we listen
  // at document level. When the user drops on a non-FilesPanel area the
  // upload still fires — acceptable since it's the only file action.
  useEffect(() => {
    const doc = (globalThis as { document?: Document }).document
    if (!doc) return
    const onDragOver = (e: DragEvent) => {
      e.preventDefault()
    }
    const onDrop = (e: DragEvent) => {
      e.preventDefault()
      const f = e.dataTransfer?.files?.[0]
      if (f) void uploadBlob(f)
    }
    doc.addEventListener('dragover', onDragOver)
    doc.addEventListener('drop', onDrop)
    return () => {
      doc.removeEventListener('dragover', onDragOver)
      doc.removeEventListener('drop', onDrop)
    }
  }, [uploadBlob])

  const share = useCallback(async (id: string) => {
    const { data, error } = await api.POST('/files/{id}/shares', {
      params: { path: { id } },
      body: { ttl_hours: 24 },
    })
    if (error) { reportError(error, 'Share failed'); return }
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
      <view
        className="h-11 rounded-[10px] bg-accent items-center justify-center mt-1"
        bindtap={busy ? undefined : pickAndUpload}
      >
        <text className="text-white text-base font-semibold">
          {busy ? t('files.uploading') : t('files.upload_file')}
        </text>
      </view>
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={busy ? undefined : uploadSample}
      >
        <text className="text-white text-base font-semibold">{t('files.upload_sample')}</text>
      </view>
      <input
        className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
        placeholder={t('files.filter_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setQuery(e.detail.value)}
      />
      <view className="mt-3 gap-2">
        {loading ? (
          <text className="text-muted text-sm py-2.5">{t('files.loading')}</text>
        ) : files.length === 0 ? (
          <text className="text-muted text-sm py-2.5">{t('files.no_files')}</text>
        ) : (
          files
            .filter((f) =>
              query.trim() ? f.filename.toLowerCase().includes(query.trim().toLowerCase()) : true,
            )
            .map((f) => (
              <view key={f.id} className="bg-card rounded-[10px] p-3 gap-1">
                <text
                  className="text-white text-[15px] font-medium"
                  bindtap={() => void share(f.id)}
                >
                  {f.filename}
                </text>
                <text className="text-muted text-xs">
                  {f.size_bytes}B · {f.content_type}
                </text>
                {f.description ? (
                  <text className="text-muted text-sm py-2.5">{f.description}</text>
                ) : null}
                <view
                  className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
                  bindtap={() => void toggleStar(f.id)}
                >
                  <text className="text-white text-base font-semibold">⭐</text>
                </view>
                <view
                  className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
                  bindtap={() => setDescEdit({ id: f.id, text: f.description ?? '' })}
                >
                  <text className="text-white text-base font-semibold">{t('files.describe')}</text>
                </view>
              </view>
            ))
        )}
      </view>
      {shareUrl ? (
        <text className="text-muted text-sm py-2.5">
          {t('files.share_label', { url: shareUrl })}
        </text>
      ) : null}
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={refreshStarred}
      >
        <text className="text-white text-base font-semibold">
          {t('files.load_starred', { count: starred.length })}
        </text>
      </view>
      {descEdit ? (
        <view>
          <input
            className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
            placeholder={t('files.description_placeholder')}
            type="text"
            bindinput={(e: { detail: { value: string } }) =>
              setDescEdit({ id: descEdit.id, text: e.detail.value })
            }
          />
          <view
            className="h-11 rounded-[10px] bg-accent items-center justify-center mt-1"
            bindtap={saveDescribe}
          >
            <text className="text-white text-base font-semibold">{t('files.save_description')}</text>
          </view>
        </view>
      ) : null}
    </view>
  )
}
