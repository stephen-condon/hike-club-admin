# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust Cloudflare Worker serving a one-page admin tool for the data
`hike-club-api` exposes. Sibling repos: `../hike-club-api` (the public read-only
API) and `../hike-club-app` (the iOS app that consumes it). This worker is the
**only writer** to the R2 bucket `hike-club-api`; the public API only reads it.

Single user, single bucket, no database. `requirements.md` for the API lives in
the sibling repo; there is no separate spec for this one beyond `openapi.yaml`.

## Commands

```bash
cargo test                              # unit + contract tests
cargo clippy --all-targets -- -D warnings
cargo fmt

cargo llvm-cov --ignore-filename-regex 'src/(lib|r2_store)\.rs$' --fail-under-lines 85

npx wrangler dev --remote --port 8788   # real R2 and real writes; no Access in front
npx wrangler deploy
```

Hooks (`core.hooksPath=.githooks`): pre-commit runs fmt + clippy, pre-push runs
tests + the coverage gate, commit-msg runs commitlint. Commits on `main` drive
`semantic-release`, so use conventional commit prefixes.

## Architecture

- **`src/validate.rs` is the trust boundary.** Everything written to the bucket
  the public API reads passes through it. Slugs are matched against
  `^[a-z0-9]+(-[a-z0-9]+)*$` by hand (no `regex` dependency — it would bloat the
  wasm binary for one pattern) and must name a **known location**: the location
  mapping doubles as the allowlist of R2 keys this worker may create.
- **`id` and `mapKey` are never read from a request body.** Both are derived
  from the path slug, which is why `HikeRequest` has no such fields. Keep it
  that way; it's what stops a write being aimed at an arbitrary object.
- **`AdminStore` (`src/store.rs`) is the test seam** — a byte-blob trait shaped
  like R2 so `R2Store` is pure translation. All handler logic lives in
  `src/admin.rs`, generic over the trait and tested against the in-memory fake.
  `src/lib.rs` and `src/r2_store.rs` can only run inside a worker and are
  excluded from the coverage gate; everything else is held to 85% lines.
- **Handlers return `Outcome`** (status + serialized body), not
  `worker::Response`, so they stay runtime-free and testable. `lib.rs` only
  translates.
- **Timestamps carry the hike's local offset.** The browser builds RFC 3339 from
  its own offset *for the chosen date*, so CDT/CST needs no timezone table
  server-side; stored strings are read back literally rather than shifted into
  the viewer's zone. This assumes the admin browses from the club's timezone.
- **`src/index.html` is the whole UI**, served via `include_str!` — no bundler,
  no framework, no static-assets binding. Mirrors how `hike-club-api` ships its
  location mapping.

## R2 layout

Shared with `hike-club-api`; see that repo for the reader's side.

| Key | Contents |
|---|---|
| `hikes/{slug}.json` | one record per **location**, overwritten on reschedule |
| `hikes/{slug}/map.png` | one map per location, shared by every record for it |
| `resources/hike-locations.json` | the mapping `GET /hike-locations` serves |

Ids carry **no date** — `start`/`end` are the only record of when a hike is. A
record left with a past `end` makes the API serve the previous hike's *observed*
weather as though it were the forecast, silently; `GET /api/hikes` returns
`stale: true` for those and the UI flags them. That failure mode is the reason
this tool exists, so don't quietly drop the flag.

## Contract testing

`openapi.yaml` is the contract and `tests/contract.rs` holds the code to it in
three directions:

1. Serialized responses validate against their schemas.
2. The spec's limits are asserted **equal** to the constants in `validate.rs`,
   and every body `validate.rs` rejects must also fail the spec — so the two
   can't drift.
3. The router's routes and the spec's paths must match both ways.

`HikeRecord`'s schema is the **cross-repo** contract: those are the exact bytes
`hike-club-api/src/models.rs` deserializes. Changing a field name here breaks
the public API, so change both together.

## Auth

Cloudflare Access, attached to the Worker in the dashboard. It rejects
unauthenticated requests before the Worker runs, so **there is deliberately no
auth code here** — don't add an API key or session handling. If a route ever
needs the caller's identity, it arrives as the `Cf-Access-Authenticated-User-Email`
header.
