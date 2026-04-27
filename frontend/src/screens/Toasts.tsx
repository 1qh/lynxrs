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
              ? 'rounded-md px-4 py-3 max-w-[360px] bg-destructive border border-destructive'
              : 'rounded-md px-4 py-3 max-w-[360px] bg-card border border-border'
          }
          bindtap={() => dismiss(t.id)}
        >
          <text
            className={
              t.kind === 'error'
                ? 'text-destructive-foreground text-[13px]'
                : 'text-card-foreground text-[13px]'
            }
          >
            {t.text}
          </text>
        </view>
      ))}
    </view>
  )
}
