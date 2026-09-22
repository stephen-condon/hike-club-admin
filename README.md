# hike-club-admin

A one-page admin tool for the data [`hike-club-api`](../hike-club-api) serves:
hike records, trail maps, and the location list. A Rust Cloudflare Worker that
writes the same R2 bucket the public API reads, with the page itself served from
the worker binary.

It replaces editing JSON by hand and running `scripts/upload-hike.sh`.

## Why it exists

Scheduling a hike used to mean editing a JSON template by hand, running an
upload script, and — for a new location — committing to the API repo and
redeploying it. Three steps, two repos, no feedback if you got one wrong.

## What it manages

| Object in R2 | Managed by |
|---|---|
| `hikes/{slug}.json` | the hike form |
| `hikes/{slug}/map.png` | the trail map upload |
| `resources/hike-locations.json` | the locations editor |

The location list used to be compiled into the API worker with `include_str!`.
It now lives in R2, with the embedded copy kept as a fallback, so adding a
location is a form rather than a deploy.

## Auth

Cloudflare Access, attached to the Worker itself in the dashboard — Workers &
Pages → `hike-club-admin` → Access → *Protect this Worker behind Access*. Since
Access binds to the Worker rather than a hostname, this covers the `workers.dev`
URL and preview URLs without a custom domain.

Access rejects unauthenticated requests before the Worker runs, so there is no
auth code, no API key and no secret in this repo. **The URL is open until that
toggle is on**; turn it on immediately after the first deploy and confirm:

```bash
curl -sI https://hike-club-admin.<subdomain>.workers.dev/ | head -1   # expect 302
```

## Develop

```bash
cargo test        # unit tests + contract tests against openapi.yaml
cargo llvm-cov --ignore-filename-regex 'src/(lib|r2_store)\.rs$' --fail-under-lines 85

npx wrangler dev --remote --port 8788   # real R2, no Access in front
npx wrangler deploy
```

`--remote` reaches the live bucket, so writes there are real writes.

## Layout

```
openapi.yaml       the API contract; tests/contract.rs holds the code to it
src/validate.rs    the trust boundary — every limit, and why
src/admin.rs       handler logic, generic over AdminStore
src/store.rs       the R2 seam, plus the in-memory fake the tests use
src/r2_store.rs    the real store (runtime-only, coverage-excluded)
src/lib.rs         router (runtime-only, coverage-excluded)
src/index.html     the whole UI
```

## Free tier

Workers (100k requests/day), R2 (10 GB, 1M class A operations/month) and Access
(50 seats) all sit inside Cloudflare's free tier at this scale.
