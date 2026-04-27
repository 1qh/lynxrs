import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'

export function WebhooksPanel() {
  const { t } = useTranslation()
  const [webhooks, setWebhooks] = useState<Array<{ id: string; url: string; enabled: boolean }>>([])
  const [webhookUrl, setWebhookUrl] = useState('')
  const [webhookSecret, setWebhookSecret] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    const { data } = await api.GET('/webhooks', {})
    if (data) setWebhooks(data as Array<{ id: string; url: string; enabled: boolean }>)
  }, [])

  const create = useCallback(async () => {
    const url = webhookUrl.trim()
    if (!url) return
    const { data } = await api.POST('/webhooks', { body: { url } })
    if (data) {
      const d = data as { secret: string }
      setWebhookSecret(d.secret)
      setWebhookUrl('')
      void refresh()
    }
  }, [webhookUrl, refresh])

  const revoke = useCallback(async (id: string) => {
    await api.DELETE('/webhooks/{id}', { params: { path: { id } } })
    void refresh()
  }, [refresh])

  return (
    <view className="gap-3">
      <view
        className="h-10 rounded-md bg-background border border-input items-center justify-center"
        bindtap={refresh}
      >
        <text className="text-foreground text-sm font-medium">{t('webhooks.load')}</text>
      </view>
      {webhooks.length > 0 ? (
        <view className="gap-2">
          {webhooks.map((w) => (
            <view key={w.id} className="rounded-md bg-card border border-border p-3 gap-2">
              <text className="text-foreground text-sm">{w.url}</text>
              <view
                className="h-9 rounded-md bg-destructive items-center justify-center"
                bindtap={() => void revoke(w.id)}
              >
                <text className="text-destructive-foreground text-sm font-medium">
                  {t('webhooks.revoke')}
                </text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
      <input
        className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
        placeholder={t('webhooks.url_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setWebhookUrl(e.detail.value)}
      />
      <view
        className="h-10 rounded-md bg-primary items-center justify-center"
        bindtap={create}
      >
        <text className="text-primary-foreground text-sm font-medium">
          {t('webhooks.register')}
        </text>
      </view>
      {webhookSecret ? (
        <text className="text-sm text-muted-foreground">
          {t('webhooks.secret_once', { secret: webhookSecret })}
        </text>
      ) : null}
    </view>
  )
}
