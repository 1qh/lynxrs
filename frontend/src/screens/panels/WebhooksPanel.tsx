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
    <view>
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={refresh}
      >
        <text className="text-white text-base font-semibold">{t('webhooks.load')}</text>
      </view>
      {webhooks.length > 0 ? (
        <view className="mt-3 gap-2">
          {webhooks.map((w) => (
            <view key={w.id} className="bg-card rounded-[10px] p-3 gap-1">
              <text className="text-white text-[15px] font-medium">{w.url}</text>
              <view
                className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
                bindtap={() => void revoke(w.id)}
              >
                <text className="text-white text-base font-semibold">{t('webhooks.revoke')}</text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
      <input
        className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
        placeholder={t('webhooks.url_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setWebhookUrl(e.detail.value)}
      />
      <view
        className="h-11 rounded-[10px] bg-accent items-center justify-center mt-1"
        bindtap={create}
      >
        <text className="text-white text-base font-semibold">{t('webhooks.register')}</text>
      </view>
      {webhookSecret ? (
        <text className="text-muted text-sm py-2.5">
          {t('webhooks.secret_once', { secret: webhookSecret })}
        </text>
      ) : null}
    </view>
  )
}
