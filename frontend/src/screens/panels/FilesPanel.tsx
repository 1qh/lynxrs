import { useCallback, useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { api } from '../../api/client.js'
import { reportError } from '../../state/toast.js'
import { useOrgContext } from '../../state/orgContext.js'
import type { components } from '../../api/schema.js'

type FileDto = components['schemas']['FileDto']

export function FilesPanel({ refreshKey }: { refreshKey: number }) {
  const { t } = useTranslation()
  const activeOrgId = useOrgContext((s) => s.activeOrgId)
  const { id: detailId } = useParams<{ id: string }>()
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
  const [preview, setPreview] = useState<FileDto | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const { data, error } = await api.GET('/files', { params: { query: { limit: 50 } } })
      if (error) reportError(error, 'Load files failed')
      if (data) {
        const d = data as { items: FileDto[]; next_cursor?: string | null }
        const items = (d.items ?? []).filter((f) =>
          activeOrgId ? f.org_id === activeOrgId : f.org_id == null,
        )
        setFiles(items)
        setCursor(d.next_cursor ?? null)
      }
    } finally {
      setLoading(false)
    }
  }, [activeOrgId])

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
        const more = (d.items ?? []).filter((f) =>
          activeOrgId ? f.org_id === activeOrgId : f.org_id == null,
        )
        setFiles((cur) => [...cur, ...more])
        setCursor(d.next_cursor ?? null)
      }
    } finally {
      setLoadingMore(false)
    }
  }, [cursor, loadingMore, activeOrgId])

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

  // Deep-link /files/:id → auto-open the preview modal for that file once
  // the list arrives (keeps a single source of truth for what's visible).
  useEffect(() => {
    if (!detailId || files.length === 0) return
    const f = files.find((x) => x.id === detailId)
    if (f) setPreview(f)
  }, [detailId, files])

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
          ...(activeOrgId ? { org_id: activeOrgId } : {}),
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
            ...(activeOrgId ? { org_id: activeOrgId } : {}),
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
    [refresh, activeOrgId],
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
                      bindtap={() => {
                        // image/pdf/text/audio/video → preview modal; others → share link.
                        const c = f.content_type
                        if (
                          c.startsWith('image/') ||
                          c === 'application/pdf' ||
                          c.startsWith('text/') ||
                          c.startsWith('audio/') ||
                          c.startsWith('video/')
                        ) {
                          setPreview(f)
                        } else void share(f.id)
                      }}
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
      {preview ? <PreviewModal file={preview} onClose={() => setPreview(null)} /> : null}
    </view>
  )
}

function PreviewModal({ file, onClose }: { file: FileDto; onClose: () => void }) {
  const base =
    (import.meta.env?.PUBLIC_API_BASE as string | undefined) ?? 'http://localhost:8088'
  const url = `${base}/api/files/${file.id}?inline=true`
  const c = file.content_type
  return (
    <view
      className="fixed inset-0 bg-background/95 items-center justify-center p-6 z-[9000]"
      bindtap={onClose}
    >
      <view className="rounded-md bg-card border border-border p-4 gap-3 max-w-[90%] max-h-[90%]">
        <text className="text-foreground text-sm font-medium">{file.filename}</text>
        {c.startsWith('image/') ? (
          <image src={url} className="rounded-md max-w-[80vw] max-h-[70vh]" />
        ) : c === 'application/pdf' ? (
          <NativeEmbed kind="iframe" url={url} className="w-[80vw] h-[70vh] rounded-md bg-background border border-border" />
        ) : c.startsWith('text/') ? (
          <TextPreview url={url} />
        ) : c.startsWith('audio/') ? (
          <NativeEmbed kind="audio" url={url} className="w-[80vw] rounded-md bg-background" />
        ) : c.startsWith('video/') ? (
          <NativeEmbed kind="video" url={url} className="w-[80vw] max-h-[70vh] rounded-md bg-background" />
        ) : null}
        <view
          className="h-9 rounded-md bg-background border border-input items-center justify-center"
          bindtap={onClose}
          aria-label="close preview"
        >
          <text className="text-foreground text-sm font-medium">close</text>
        </view>
      </view>
    </view>
  )
}

/**
 * Render a native HTML element (iframe/audio/video) by appending it directly
 * to document.body with fixed positioning. Lynx custom elements aren't real
 * DOM nodes that accept `<iframe>` as a child, so we sidestep the synthetic
 * tree and overlay the embed at a fixed center-of-screen position.
 */
function NativeEmbed({
  kind,
  url,
}: {
  kind: 'iframe' | 'audio' | 'video'
  url: string
  className?: string
}) {
  useEffect(() => {
    const doc = (globalThis as { document?: Document }).document
    if (!doc) return
    const el = doc.createElement(kind) as HTMLIFrameElement | HTMLAudioElement | HTMLVideoElement
    if (kind === 'iframe') (el as HTMLIFrameElement).src = url
    else {
      ;(el as HTMLMediaElement).src = url
      ;(el as HTMLMediaElement).controls = true
    }
    el.style.position = 'fixed'
    el.style.left = '50%'
    el.style.top = '50%'
    el.style.transform = 'translate(-50%, -50%)'
    el.style.width = '80vw'
    el.style.maxHeight = '70vh'
    el.style.border = '0'
    el.style.borderRadius = '8px'
    el.style.zIndex = '9100'
    el.style.background = '#000'
    doc.body.appendChild(el)
    return () => {
      try {
        doc.body.removeChild(el)
      } catch {}
    }
  }, [kind, url])
  return (
    <view className="w-[80vw] h-[70vh] rounded-md bg-background border border-border items-center justify-center">
      <text className="text-muted-foreground text-sm">loading {kind}…</text>
    </view>
  )
}

function TextPreview({ url }: { url: string }) {
  const [text, setText] = useState<string>('loading…')
  useEffect(() => {
    void fetch(url, { credentials: 'include' })
      .then((r) => r.text())
      .then((s) => setText(s.length > 8000 ? s.slice(0, 8000) + '\n…(truncated)' : s))
      .catch((e) => setText(`error: ${String(e)}`))
  }, [url])
  return (
    <view className="w-[80vw] h-[70vh] rounded-md bg-background border border-border p-3 overflow-auto">
      <text className="text-foreground text-xs font-mono whitespace-pre-wrap">{text}</text>
    </view>
  )
}
