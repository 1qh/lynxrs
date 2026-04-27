import type { Config } from 'tailwindcss'
import preset from '@lynx-js/tailwind-preset'

export default {
  presets: [preset],
  content: ['./src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        bg: '#0b1020',
        panel: '#10172a',
        card: '#1a2340',
        muted: '#7a8299',
        accent: '#3b82f6',
        accent2: '#3a4a8a',
        border: '#2a355a',
        danger: '#ff6b7a',
        warn: '#f5c46b',
      },
    },
  },
} satisfies Config
