/**
 * Hand-rolled markdown → Lynx primitives. The Lynx main-thread JS engine
 * rejects react-markdown's Unicode-class regex, so we use only ASCII regex
 * and emit `<view>`/`<text>` nodes directly.
 *
 * Supported: fenced code blocks, inline code, **bold**, _italic_, # headings,
 * - lists, > blockquotes, [text](url) links, [N] citations, blank-line
 * paragraph breaks. Anything else passes through as plain text.
 */
import type { ReactNode } from '@lynx-js/react'

type Block =
  | { kind: 'code'; lang: string; body: string }
  | { kind: 'h'; level: 1 | 2 | 3; text: string }
  | { kind: 'quote'; text: string }
  | { kind: 'list'; items: string[] }
  | { kind: 'p'; text: string }

function tokenizeBlocks(src: string): Block[] {
  const out: Block[] = []
  const lines = src.split('\n')
  let i = 0
  while (i < lines.length) {
    const line = lines[i]!
    // Fenced code block.
    if (line.startsWith('```')) {
      const lang = line.slice(3).trim()
      const start = i + 1
      let end = start
      while (end < lines.length && !lines[end]!.startsWith('```')) end++
      out.push({ kind: 'code', lang, body: lines.slice(start, end).join('\n') })
      i = end + 1
      continue
    }
    // Heading.
    const h = /^(#{1,3})\s+(.*)$/.exec(line)
    if (h) {
      out.push({ kind: 'h', level: h[1]!.length as 1 | 2 | 3, text: h[2]! })
      i++
      continue
    }
    // Blockquote (one line at a time).
    if (line.startsWith('> ')) {
      out.push({ kind: 'quote', text: line.slice(2) })
      i++
      continue
    }
    // List (consecutive `- ` / `* ` / `N. ` lines).
    if (/^([-*]|\d+\.)\s+/.test(line)) {
      const items: string[] = []
      while (i < lines.length && /^([-*]|\d+\.)\s+/.test(lines[i]!)) {
        items.push(lines[i]!.replace(/^([-*]|\d+\.)\s+/, ''))
        i++
      }
      out.push({ kind: 'list', items })
      continue
    }
    // Blank line: skip.
    if (line.trim() === '') { i++; continue }
    // Paragraph: gather until blank or block-starting line.
    const buf: string[] = [line]
    i++
    while (
      i < lines.length &&
      lines[i]!.trim() !== '' &&
      !lines[i]!.startsWith('```') &&
      !/^(#{1,3})\s+/.test(lines[i]!) &&
      !lines[i]!.startsWith('> ') &&
      !/^([-*]|\d+\.)\s+/.test(lines[i]!)
    ) {
      buf.push(lines[i]!)
      i++
    }
    out.push({ kind: 'p', text: buf.join(' ') })
  }
  return out
}

/** Inline tokens: bold/italic/code/link/citation/text. Regex restricted to
 *  ASCII so Lynx's main-thread engine accepts the patterns. */
type Inline =
  | { t: 'text'; v: string }
  | { t: 'bold'; v: string }
  | { t: 'italic'; v: string }
  | { t: 'code'; v: string }
  | { t: 'link'; text: string; href: string }
  | { t: 'cite'; n: string }

function inlineSplit(src: string): Inline[] {
  const out: Inline[] = []
  let rest = src
  while (rest.length > 0) {
    // Inline code.
    const code = /`([^`]+)`/.exec(rest)
    const bold = /\*\*([^*]+)\*\*/.exec(rest)
    const italic = /(?:^|[^*])_([^_]+)_/.exec(rest)
    const link = /\[([^\]]+)\]\(([^)]+)\)/.exec(rest)
    const cite = /\[(\d{1,3})\](?!\()/.exec(rest)
    // Pick the earliest match.
    const candidates = [code, bold, italic, link, cite].filter((m) => m) as RegExpExecArray[]
    if (candidates.length === 0) {
      out.push({ t: 'text', v: rest })
      break
    }
    candidates.sort((a, b) => a.index - b.index)
    const m = candidates[0]!
    if (m.index > 0) out.push({ t: 'text', v: rest.slice(0, m.index) })
    if (m === code) out.push({ t: 'code', v: m[1]! })
    else if (m === bold) out.push({ t: 'bold', v: m[1]! })
    else if (m === italic) out.push({ t: 'italic', v: m[1]! })
    else if (m === link) out.push({ t: 'link', text: m[1]!, href: m[2]! })
    else if (m === cite) out.push({ t: 'cite', n: m[1]! })
    rest = rest.slice(m.index + m[0].length)
  }
  return out
}

function renderInline(parts: Inline[]): ReactNode {
  return parts.map((p, i) => {
    if (p.t === 'bold') return <text key={i} className="font-semibold">{p.v}</text>
    if (p.t === 'italic') return <text key={i} className="italic">{p.v}</text>
    if (p.t === 'code') return (
      <text key={i} className="font-mono bg-secondary text-secondary-foreground rounded px-1">{p.v}</text>
    )
    if (p.t === 'link') return <text key={i} className="text-primary underline">{p.text}</text>
    if (p.t === 'cite') return (
      <text key={i} className="text-primary font-mono text-xs">[{p.n}]</text>
    )
    return <text key={i}>{p.v}</text>
  })
}

export function LynxMarkdown({ source }: { source: string }) {
  const blocks = tokenizeBlocks(source)
  return (
    <view className="gap-2">
      {blocks.map((b, i) => {
        if (b.kind === 'code') return (
          <view key={i} className="rounded-md bg-secondary border border-border p-2">
            {b.lang ? <text className="text-xs text-muted-foreground mb-1">{b.lang}</text> : null}
            <text className="font-mono text-foreground text-xs whitespace-pre-wrap">{b.body}</text>
          </view>
        )
        if (b.kind === 'h') {
          const cls = b.level === 1 ? 'text-base font-bold' : b.level === 2 ? 'text-sm font-semibold' : 'text-sm font-medium'
          return <text key={i} className={`${cls} text-foreground`}>{b.text}</text>
        }
        if (b.kind === 'quote') return (
          <view key={i} className="border-l-2 border-border pl-3">
            <text className="text-muted-foreground text-sm italic">{b.text}</text>
          </view>
        )
        if (b.kind === 'list') return (
          <view key={i} className="gap-1 pl-2">
            {b.items.map((it, j) => (
              <view key={j} className="flex-row gap-2">
                <text className="text-muted-foreground text-sm">•</text>
                <text className="text-foreground text-sm flex-1">{renderInline(inlineSplit(it))}</text>
              </view>
            ))}
          </view>
        )
        // p
        return <text key={i} className="text-foreground text-sm">{renderInline(inlineSplit(b.text))}</text>
      })}
    </view>
  )
}
