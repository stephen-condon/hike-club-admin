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

Records carry **no date**. hike-club-app holds each hike's date and sends it as
query parameters when it fetches trail info; this repo neither stores nor
requires `start`/`end`.

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

## LID
- Mode: Full
- Version: 1.3.0

## Linked-Intent Development (MANDATORY)

**Consult the `linked-intent-dev` skill for ALL code changes.** All changes flow through the arrow of intent in one direction:

```
HLD → LLDs → EARS → Tests → Code
```

- **New features and refactors**: full six-phase workflow (HLD check → LLD check/draft → EARS → intent-narrowing edge audit → tests-first → code).
- **Bug fixes**: walk the arrow like any other change — find where behavior diverged from intent and cascade from there. No short-circuit.
- **If unsure**: use the full workflow.

Stop after each phase for user review. **Docs carry current intent, written to be read cold** — write each doc as if authored fresh today, from current intent alone: no narration of how it changed, no meaning that needs the conversation that produced it, no rebuttals to questions only a past discussion raised. Rationale, considered alternatives, and constraints a fresh author would independently write stay; record rejected alternatives and why in the LLD's Decisions & Alternatives table, not as asides in body prose.

**Memory vs. intent.** Before saving durable project knowledge to agent or tool memory, test whether it is project *intent* — would a fresh agent, in any tool, next session, need it to build this system correctly? If yes, record it in the arrow (HLD / LLD / EARS / decision doc), which travels and cascades — not in private, per-tool memory, where intent escapes the arrow. Knowledge about the user or how they like to work stays in memory.

### Navigation

| What you need | Where to look |
|---|---|
| High-level design | `docs/high-level-design.md` |
| Design tree (sub-HLDs, LLDs, their specs) | `docs/intent/` — one folder per node |
| EARS specs | beside each design doc as `{node}-specs.md` in the node's folder under `docs/intent/` |
| Decision docs | `docs/decisions/` (project-level) and `docs/intent/<segment>/decisions/` |
| Arrow of intent overlay | `docs/arrows/index.yaml` and per-segment docs in `docs/arrows/` |

### Terminology

- **HLD**: High-Level Design — single project-level doc at `docs/high-level-design.md`.
- **LLD**: Low-Level Design — detailed component design doc in `docs/intent/`. The design layer is a recursive tree: the root is the HLD, leaf LLDs own EARS, and a component deep enough to outgrow one doc becomes a sub-HLD (HLD-shaped, owns no EARS) with children beneath it. "HLD" and "LLD" are roles by position; depth-2 (one HLD over flat leaf LLDs) is the default.
- **EARS**: Easy Approach to Requirements Syntax — structured one-line requirements beside each design doc as `{node}-specs.md` in the node's folder under `docs/intent/`. IDs are path-concatenated — the root-to-leaf path of the owning segment plus a number — so a prefix grep gathers a subtree. Markers: `[x]` implemented, `[ ]` active gap, `[D]` deferred.
- **Arrow**: the unidirectional chain from vision to code (HLD → LLDs → EARS → Tests → Code). Strictly a DAG of intent.
- **Arrow segment**: the territory owned by one leaf LLD — the LLD itself plus the specs, tests, and code that cite its EARS IDs. The boundary is the leaf prefix. Within-segment cascade is free; across-segment cascade pauses.
- **Cascade**: propagating a change downstream through the arrow so adjacent levels stay coherent.

### Code annotations

Annotate code and tests with `@spec` comments citing EARS IDs:

```
// @spec AUTH-UI-001, AUTH-UI-002
```

Place the annotation at the *entry point of the behavior's implementation graph* — the topmost function or module owning the specified behavior, not every helper. When a behavior spans multiple subsystems (UI + API + database, for example), annotate at the entry point in each subsystem. Tests follow the same rule: annotate the test that directly exercises the spec, not every inner assertion.
