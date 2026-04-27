import { useToasts } from '../state/toast.js'

export function Toasts() {
  const toasts = useToasts((s) => s.toasts)
  const dismiss = useToasts((s) => s.dismiss)
  if (toasts.length === 0) return null
  return (
    <view className="fixed bottom-5 right-5 gap-2 z-[9999]">
      {toasts.map((t) => (
        <view
          key={t.id}
          className={
            t.kind === 'error'
              ? 'rounded-lg px-4 py-3 max-w-[360px] bg-[#4a1a20] border border-[#8a3340]'
              : 'rounded-lg px-4 py-3 max-w-[360px] bg-card border border-accent2'
          }
          bindtap={() => dismiss(t.id)}
        >
          <text className="text-[#f5d5d8] text-[13px]">{t.text}</text>
        </view>
      ))}
    </view>
  )
}
