import { useCallback, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'

export function WebhooksPanel() {
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
      <view className="Button ButtonGhost" bindtap={refresh}>
        <text className="ButtonText">Load webhooks</text>
      </view>
      {webhooks.length > 0 ? (
        <view className="WebhookList">
          {webhooks.map((w) => (
            <view key={w.id} className="WebhookRow">
              <text className="WebhookUrl">{w.url}</text>
              <view className="Button ButtonGhost" bindtap={() => void revoke(w.id)}>
                <text className="ButtonText">revoke</text>
              </view>
            </view>
          ))}
        </view>
      ) : null}
      <input
        className="Input"
        placeholder="https://your-host/hook"
        type="text"
        bindinput={(e: { detail: { value: string } }) => setWebhookUrl(e.detail.value)}
      />
      <view className="Button" bindtap={create}>
        <text className="ButtonText">Register webhook</text>
      </view>
      {webhookSecret ? (
        <text className="Muted">webhook secret (copy now, shown once): {webhookSecret}</text>
      ) : null}
    </view>
  )
}
