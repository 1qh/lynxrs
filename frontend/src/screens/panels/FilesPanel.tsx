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

  const filtered = files.filter((f) =>
    query.trim() ? f.filename.toLowerCase().includes(query.trim().toLowerCase()) : true,
  )

  return (
    <view className="gap-3">
      <view className="flex-row gap-2">
        <view
          className="h-10 flex-1 rounded-md bg-primary items-center justify-center"
          bindtap={busy ? undefined : pickAndUpload}
        >
          <text className="text-primary-foreground text-sm font-medium">
            {busy ? t('files.uploading') : t('files.upload_file')}
          </text>
        </view>
        <view
          className="h-10 rounded-md bg-background border border-input items-center justify-center px-4"
          bindtap={busy ? undefined : uploadSample}
        >
          <text className="text-foreground text-sm font-medium">
            {t('files.upload_sample')}
          </text>
        </view>
      </view>
      <input
        className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
        placeholder={t('files.filter_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setQuery(e.detail.value)}
      />
      <view className="gap-2">
        {loading ? (
          <text className="text-sm text-muted-foreground">{t('files.loading')}</text>
        ) : filtered.length === 0 ? (
          <text className="text-sm text-muted-foreground">{t('files.no_files')}</text>
        ) : (
          filtered.map((f) => (
            <view key={f.id} className="rounded-md bg-card border border-border p-3 gap-2">
              <text
                className="text-foreground text-sm font-medium"
                bindtap={() => void share(f.id)}
              >
                {f.filename}
              </text>
              <text className="text-xs text-muted-foreground">
                {f.size_bytes}B · {f.content_type}
              </text>
              {f.description ? (
                <text className="text-sm text-muted-foreground">{f.description}</text>
              ) : null}
              <view className="flex-row gap-2">
                <view
                  className="h-8 rounded-md bg-background border border-input items-center justify-center px-3"
                  bindtap={() => void toggleStar(f.id)}
                >
                  <text className="text-foreground text-sm">⭐</text>
                </view>
                <view
                  className="h-8 rounded-md bg-background border border-input items-center justify-center px-3"
                  bindtap={() => setDescEdit({ id: f.id, text: f.description ?? '' })}
                >
                  <text className="text-foreground text-sm">{t('files.describe')}</text>
                </view>
              </view>
            </view>
          ))
        )}
      </view>
      {shareUrl ? (
        <text className="text-sm text-muted-foreground">
          {t('files.share_label', { url: shareUrl })}
        </text>
      ) : null}
      <view
        className="h-10 rounded-md bg-background border border-input items-center justify-center"
        bindtap={refreshStarred}
      >
        <text className="text-foreground text-sm font-medium">
          {t('files.load_starred', { count: starred.length })}
        </text>
      </view>
      {descEdit ? (
        <view className="rounded-md bg-card border border-border p-3 gap-2">
          <input
            className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
            placeholder={t('files.description_placeholder')}
            type="text"
            bindinput={(e: { detail: { value: string } }) =>
              setDescEdit({ id: descEdit.id, text: e.detail.value })
            }
          />
          <view
            className="h-10 rounded-md bg-primary items-center justify-center"
            bindtap={saveDescribe}
          >
            <text className="text-primary-foreground text-sm font-medium">
              {t('files.save_description')}
            </text>
          </view>
        </view>
      ) : null}
    </view>
  )
}
