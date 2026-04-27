# AGENTS.md — Lynx side

You are working in a ReactLynx app on `rspeedy`. Read [`frontend/README.md`](./README.md) first for stack + layout. Hard rules in [`../CONTRIBUTING.md`](../CONTRIBUTING.md).

## Required reading

- Lynx — [llms.txt](https://lynxjs.org/next/llms.txt) (entry point for all Lynx docs)
- Rsbuild — <https://rsbuild.rs/llms.txt>
- Rspack — <https://rspack.rs/llms.txt>

## simu-specific patterns

| Topic | Rule |
|---|---|
| Markdown | use `src/lib/lynxMarkdown.tsx` — Lynx engine rejects `react-markdown`'s Unicode-class regex |
| Inputs | uncontrolled — `inputRef` + `bindinput`; no `value` prop |
| Navigation | `<view bindtap={() => navigate(path)}>` — no `<a>` / `NavLink` |
| Fetch | guard with `(globalThis as { fetch?: typeof fetch }).fetch` — Lynx native may lack it |
| Upload | JSON + base64 path (`/api/files/upload_json`) — no `FormData` / `Blob` |
| Streaming | `EventSource`-style SSE reader on `fetch().body.getReader()`, `AbortController` for stop |
| Realtime | `useEvents([...kinds], handler)` over `/events/ws` (tokio broadcast bus) |
| Routing | `CrossRouter` (`MemoryRouter` native / `HashRouter` web). Avoid `<Route index><Navigate/></Route>` — render directly |
| State | zustand stores under `src/state/`; persist via `localStorage` keys `simu.*` |
| i18n | always `t(...)` — never hardcode strings; update both `en.json` + `vi.json` |
| Service worker | `web-host/sw.js` — never cache `/api/*` |

## Commands

```bash
bun run dev              # rspeedy dev
bun run build            # production bundles
bun run typecheck
bunx rspeedy inspect
```
