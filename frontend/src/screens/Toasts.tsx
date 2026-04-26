import { useToasts } from '../state/toast.js'

export function Toasts() {
  const toasts = useToasts((s) => s.toasts)
  const dismiss = useToasts((s) => s.dismiss)
  if (toasts.length === 0) return null
  return (
    <view className="ToastStack">
      {toasts.map((t) => (
        <view
          key={t.id}
          className={t.kind === 'error' ? 'Toast ToastError' : 'Toast ToastInfo'}
          bindtap={() => dismiss(t.id)}
        >
          <text className="ToastText">{t.text}</text>
        </view>
      ))}
    </view>
  )
}
