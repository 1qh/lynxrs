import { useCallback, useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import { reportError } from '../../state/toast.js'
import type { components } from '../../api/schema.js'

type FileDto = components['schemas']['FileDto']

export function FilesPanel({ refreshKey }: { refreshKey: number }) {
  const { t } = useTranslation()
  const [files, setFiles] = useState<FileDto[]>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  const [shareUrl, setShareUrl] = useState<string | null>(null)
  const [starred, setStarred] = useState<FileDto[]>([])
  const [descEdit, setDescEdit] = useState<{ id: string; text: string } | null>(null)
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set())

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const { data, error } = await api.GET('/files', { params: { query: { limit: 50 } } })
      if (error) reportError(error, 'Load files failed')
      if (data) {
        const d = data as { items: FileDto[]; next_cursor?: string | null }
        setFiles(d.items ?? [])
        setCursor(d.next_cursor ?? null)
      }
    } finally {
      setLoading(false)
    }
  }, [])

  const loadMore = useCallback(async () => {
    if (!cursor || loadingMore) return
    setLoadingMore(true)
    try {
      const { data, error } = await api.GET('/files', {
        params: { query: { limit: 50, cursor } },
      })
      if (error) reportError(error, 'Load more failed')
      if (data) {
        const d = data as { items: FileDto[]; next_cursor?: string | null }
        setFiles((cur) => [...cur, ...(d.items ?? [])])
        setCursor(d.next_cursor ?? null)
      }
    } finally {
      setLoadingMore(false)
    }
  }, [cursor, loadingMore])

  const toggleSelect = useCallback((id: string) => {
    setSelected((cur) => {
      const next = new Set(cur)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }, [])

  const clearSelection = useCallback(() => setSelected(new Set()), [])

  const bulkDelete = useCallback(async () => {
    const ids = Array.from(selected)
    if (ids.length === 0) return
    const { error } = await api.POST('/files/bulk', { body: { action: 'delete', ids } })
    if (error) reportError(error, 'Bulk delete failed')
    clearSelection()
    void refresh()
  }, [selected, clearSelection, refresh])

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
      {selected.size > 0 ? (
        <view className="flex-row items-center gap-2 rounded-md bg-secondary border border-border px-3 py-2">
          <text className="flex-1 text-secondary-foreground text-sm">
            {t('files.selected', { count: selected.size })}
          </text>
          <view
            className="h-8 rounded-md bg-background border border-input items-center justify-center px-3"
            bindtap={clearSelection}
          >
            <text className="text-foreground text-sm">{t('files.clear')}</text>
          </view>
          <view
            className="h-8 rounded-md bg-destructive items-center justify-center px-3"
            bindtap={() => void bulkDelete()}
          >
            <text className="text-destructive-foreground text-sm font-medium">
              {t('files.delete')}
            </text>
          </view>
        </view>
      ) : null}
      <view className="gap-2">
        {loading ? (
          <text className="text-sm text-muted-foreground">{t('files.loading')}</text>
        ) : filtered.length === 0 ? (
          <view className="rounded-md border border-dashed border-border p-6 items-center gap-2">
            <text className="text-foreground text-base font-medium">{t('files.no_files')}</text>
            <text className="text-sm text-muted-foreground text-center">
              {t('files.no_files_hint')}
            </text>
          </view>
        ) : (
          filtered.map((f) => {
            const isSelected = selected.has(f.id)
            return (
              <view
                key={f.id}
                className={
                  isSelected
                    ? 'rounded-md bg-secondary border border-primary p-3 gap-2'
                    : 'rounded-md bg-card border border-border p-3 gap-2'
                }
              >
                <view className="flex-row items-start gap-2">
                  <view
                    className={
                      isSelected
                        ? 'w-5 h-5 rounded bg-primary border border-primary items-center justify-center'
                        : 'w-5 h-5 rounded bg-background border border-input items-center justify-center'
                    }
                    bindtap={() => toggleSelect(f.id)}
                  >
                    {isSelected ? (
                      <text className="text-primary-foreground text-xs">✓</text>
                    ) : null}
                  </view>
                  <view className="flex-1 gap-1">
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
                  </view>
                </view>
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
            )
          })
        )}
      </view>
      {cursor ? (
        <view
          className="h-10 rounded-md bg-background border border-input items-center justify-center"
          bindtap={loadingMore ? undefined : () => void loadMore()}
        >
          <text className="text-foreground text-sm font-medium">
            {loadingMore ? t('files.loading') : t('files.load_more')}
          </text>
        </view>
      ) : null}
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
