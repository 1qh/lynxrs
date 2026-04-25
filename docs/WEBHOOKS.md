# Webhook receivers — verify your simu signature

Every outbound webhook from simu carries an HMAC-SHA256 signature in the
`x-simu-signature` header. Verify it before trusting the body.

## Header layout

```
x-simu-signature: sha256=<hex digest>
x-simu-timestamp: <unix seconds>
x-simu-event:     file_created | file_deleted | …
x-simu-delivery:  <uuid>
```

The signed payload is `{timestamp}.{raw_body}`. Replay attacks are mitigated
by rejecting requests with timestamps more than 300 seconds in the past.

## Reference receivers

### Node (TypeScript)

```ts
import crypto from 'node:crypto'

function verify(req: Request, secret: string, rawBody: Buffer): boolean {
  const sig = req.headers['x-simu-signature'] as string | undefined
  const ts = req.headers['x-simu-timestamp'] as string | undefined
  if (!sig?.startsWith('sha256=') || !ts) return false
  if (Math.abs(Date.now() / 1000 - Number(ts)) > 300) return false

  const expected = crypto
    .createHmac('sha256', secret)
    .update(`${ts}.${rawBody.toString('utf8')}`)
    .digest('hex')

  // Constant-time compare
  const a = Buffer.from(sig.slice(7), 'hex')
  const b = Buffer.from(expected, 'hex')
  return a.length === b.length && crypto.timingSafeEqual(a, b)
}
```

### Python (FastAPI)

```python
import hmac, hashlib, time

def verify(request, secret: str, raw_body: bytes) -> bool:
    sig = request.headers.get("x-simu-signature", "")
    ts  = request.headers.get("x-simu-timestamp", "")
    if not sig.startswith("sha256=") or not ts.isdigit():
        return False
    if abs(int(time.time()) - int(ts)) > 300:
        return False
    expected = hmac.new(
        secret.encode(),
        f"{ts}.{raw_body.decode('utf-8')}".encode(),
        hashlib.sha256,
    ).hexdigest()
    return hmac.compare_digest(sig[7:], expected)
```

### Rust (axum receiver)

```rust
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;

fn verify(secret: &str, ts: &str, sig_header: &str, body: &[u8]) -> bool {
    let Some(hex_sig) = sig_header.strip_prefix("sha256=") else { return false };
    let now = chrono::Utc::now().timestamp();
    let Ok(ts_i) = ts.parse::<i64>() else { return false };
    if (now - ts_i).abs() > 300 { return false }

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(ts.as_bytes());
    mac.update(b".");
    mac.update(body);
    let Ok(provided) = hex::decode(hex_sig) else { return false };
    mac.verify_slice(&provided).is_ok()
}
```

## When to retry

simu retries failed deliveries with exponential backoff up to 5 attempts. To
be idempotent, key your handler on `x-simu-delivery` (a uuid v7 — sortable).

## Rotation

Rotate the per-webhook secret by issuing `POST /api/webhooks/{id}/rotate`
(coming soon). Until then, delete + recreate the webhook to invalidate the
old secret.
