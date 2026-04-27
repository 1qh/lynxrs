# simu-frontend (Lynx)

ReactLynx app via rspeedy. Builds two bundles from one source: `main.lynx.bundle` (native iOS/Android via LynxExplorer) and `main.web.bundle` (web via shadow-DOM `<lynx-view>`). Same code, vertical phone shape on every target.

## Stack

- `@lynx-js/react` 0.119+ — JSX with `<view>` `<text>` `<image>` `<input>` primitives (no HTML)
- `@lynx-js/rspeedy` 0.14 + `@lynx-js/tailwind-preset` (HSL CSS-var shadcn tokens, dark-class)
- `react-router` v6.30 — `MemoryRouter` on Lynx native, `HashRouter` on web (`CrossRouter` shim)
- `zustand` — auth, theme, toast, projects, files, orgContext
- `openapi-fetch` — typed client generated from `/api-docs/openapi.json`
- `react-i18next` — EN + VI under `src/i18n/locales/`
- Service worker (`web-host/sw.js`) — cache-first shell, network-only `/api`, push handler
- Playwright — shadow-DOM helpers (`waitForText`, `fillInput`, `tapText`)

## Layout

```mermaid
flowchart TD
  App["App.tsx · header · CrossRouter · CommandPalette · ShortcutsHelp"]
  Home["Home.tsx · bottom-tab shell · Outlet"]
  Tabs["tabs · Chat / Files / Orgs / Settings + More drawer"]
  App --> Home --> Tabs

  subgraph Panels["src/screens/panels/"]
    Chat["ChatPanel · SSE reader · markdown · projects"]
    Files["FilesPanel · upload · versions · share"]
    Orgs["OrgsPanel"]
    Settings["SettingsPanel · Profile · Notifications · MFA · Webhooks"]
    Audit["AuditPanel"] · Trash["TrashPanel"] · Admin["AdminPanel"]
  end
  Tabs --> Panels

  subgraph State["src/state/ (zustand)"]
    auth · theme · toast · projects · filesState · orgContext
  end
  Panels --> State
  Panels -->|openapi-fetch| API["/api/*"]
  Panels -->|EventSource| SSE["/api/conversations/:id/stream"]
  Panels -->|WebSocket| WS["/events/ws"]
```

## Lynx-specific gotchas

- Main-thread JS engine **rejects Unicode-class regex** → `react-markdown` unusable. Hand-rolled parser at `src/lib/lynxMarkdown.tsx` (ASCII-only).
- No `FormData` / `Blob` / native `fetch` guarantee → upload via JSON+base64; feature-detect `globalThis.fetch`.
- `<input>` does not support controlled `value` — use `inputRef` + `bindinput`.
- No `<a>` / `NavLink` — emit `<view bindtap={…}>` with `useNavigate`.
- Routing race: `<Route index>` redirects can drop in `MemoryRouter` — render the panel directly.

## Commands

```bash
bun install --frozen-lockfile
bun run dev            # rspeedy dev · :3000 · web preview at /__web_preview?casename=main.web.bundle
bun run build          # both bundles
bun run preview
bun run typecheck      # tsc --noEmit (strict + noUncheckedIndexedAccess + exactOptionalPropertyTypes)
bun run openapi:codegen   # regen src/api/schema.ts from running backend
bun run test:e2e       # playwright
bunx rspeedy inspect   # rspack/rsbuild config dump
```

## i18n

Add a key under `src/i18n/locales/{en,vi}.json` → use `const { t } = useTranslation()`. Both files must stay in lockstep.

## E2E

See [`docs/TESTING.md`](../docs/TESTING.md) for the matrix and runbook.
