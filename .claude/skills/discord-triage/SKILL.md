---
name: discord-triage
description: Triage Taigi Keyboard Discord #general chat into the #issues forum. Two phases - `scan` writes a review list (triage.md) of messages that look like bug reports or feature requests, skipping ones the bot already handled; `apply` executes the approved rows (create #issues post with message link, reply under the #general message, mark fixed posts with the version + close, fix tags). Requires the `discord` MCP server (discord-mcp). Args - `scan [n]` or `apply <triage.md>`.
disable-model-invocation: false
---

# Discord Triage

Turn #general chat into tracked #issues forum posts. Backed by the `discord` MCP server
(`~/Workspace/discord-mcp`; tools `read_channel`, `list_posts`, `read_post`, `create_post`,
`reply_post`, `list_tags`, `set_tags`/`add_tags`/`remove_tags`, `close_post`, `reopen_post`).

Tag `done` = closed marker (Discord forums have no open/closed filter; USER 2026-09-18 chose
tag filtering): every close adds `done`, every reopen removes it. Tag `drop` = USER-only.

**Never write to Discord in `scan`. Only `apply` writes, and only rows the USER approved.**

## Config

`channels.json` (this dir): `general` / `issues` channel IDs, `bot_username` (the MCP bot's
Discord username — used to recognise its own replies). Any value empty → ask the USER once,
then write it back to the file.

MCP not registered (`claude mcp list` has no `discord`) → tell the USER to run
`claude mcp add -s user discord -e DISCORD_BOT_TOKEN=<token> -- uv --directory ~/Workspace/discord-mcp run discord-mcp`
themselves (token is theirs), then restart the session.

## `scan [n]` — build the review list (read-only)

1. **Incremental by default.** `state.json` (this dir) holds `last_seen`, the newest #general
   message ID the USER has already reviewed. Read `read_channel(general, limit=100,
   after=<last_seen>)` and page forward (`after=<newest id>`) until a page comes back short.
   No `state.json` or `n` given → full scan: `read_channel(general, limit=100)`, page back
   with `before=<oldest id>` until `n` messages or the channel ends.
2. **Skip already-handled messages**: collect `reply_to` of every message whose `author ==
   bot_username`; any message whose `id` is in that set is done. Second dedup source: message
   links (`https://discord.com/channels/<guild>/<channel>/<id>`) found in the first message of
   every `list_posts(issues, include_archived=true)` post (`read_post` each) — only needed for
   a full scan; an incremental scan checks links of posts created after `last_seen` only.
3. **Candidate = 疑似需求, loose** (USER 2026-09-18: 寬鬆, the USER reviews the list). Keep a
   message when it reports something broken or asks for a capability: bug / feature / "希望",
   "可以…嗎", "能不能", "壞掉", "沒反應", "打不出來", "建議", stack of screenshots + complaint.
   Drop pure discussion, greetings, thanks, answers to someone else's question, and bot output.
   Merge a reporter's consecutive messages (same author, < 10 min apart) into one row; link the
   first message.
4. **Fixed check**: for each candidate, grep `changelog/*.md` and `docs/architecture/dogfood-checklist.md`
   for the symptom's keywords. A clear hit → `fixed` + the changelog file's version; no hit or
   unsure → leave the version column empty. Never guess a version.
5. **Tags**: `list_tags(issues)` once; pick the best-fitting existing tag names per row
   (platform + kind, e.g. `ios`, `android`, `bug`, `feature`). Never propose `drop`; `done` is
   added by `apply` on close, not proposed here.
6. Write `triage.md` to the scratchpad dir and print its path. One row per candidate:

   ```
   | # | action | link | author | date | summary (EN, ≤ 1 line) | tags | fixed in | note |
   ```
   `action` ∈ `create` (new #issues post) · `fixed` (create post, reply fixed, close) ·
   `skip`. Existing #issues posts that match a candidate get `action = exists` with the post
   ID in `note`, so `apply` replies under the #general message without creating a duplicate.

   Record the newest scanned message ID at the top of `triage.md` (`last_seen: <id>`) so
   `apply` can write it to `state.json`.

   **Open-post audit (every scan, USER 2026-09-18)**: for every non-archived #issues post from
   `list_posts(issues)` **not tagged `done`** (skip those without `read_post`; USER
   2026-09-18), `read_post` for the body and decide whether it is already fixed:
   - grep `changelog/*.md` for the symptom → hit = released: `fixed in` = that file's version.
   - no changelog hit → `git log --oneline main --grep=<keyword>` + project memory
     (`MEMORY.md` active rounds): a MERGED PR that resolves the symptom = fixed, unreleased.
     `fixed in` = the train's in-tree (unreleased) version: newest `chore(release): bump
     <train> version to X.Y.Z` commit on `main`; none after the latest `mobile-*` /
     `desktop-*` tag → tag +1 patch. Mark `(next, proposed)` in `note`. Engine fixes touch
     both trains — name both versions. Verify the fix is NOT inside the latest tag first
     (`git merge-base --is-ancestor <merge sha> <tag>`), else it is released and belongs to
     that changelog.
     The USER confirms or overwrites the version; release scope stays the USER's call.
   - neither → not fixed, no row.
   Emit one `close` row per fixed post below the candidate table:

   ```
   | # | action | post | title | fixed in | evidence (changelog file / PR #) | note |
   ```
   Include in the same list inconsistent posts: open (non-archived) but tagged `done` (propose
   `close`, no re-audit), or tagged `drop` — same review rule applies. Archived posts are never
   scanned.

7. Stop. Report the counts (scanned / skipped as handled / candidates) and the file path.
   USER edits the file (change `action`, fill `fixed in`, delete rows), then runs `apply`.
   Zero candidates and no `close` / `retag` rows → no `apply` will follow: write
   `state.json` now (local file, still no Discord write) and say so.

## `apply <triage.md>` — execute approved rows (writes)

Process rows top-down; on any Discord error stop, report the row, do not retry blindly.

| action | steps |
|---|---|
| `create` | `create_post(issues, title=summary, content=<template A>, tags)` → `reply_post(general, <template B>, reply_to=<msg id>)` |
| `fixed` | same as `create`, then `reply_post(post, <template C>)` → `add_tags(post, ["done"])` → `close_post(post)` |
| `exists` | `reply_post(general, <template B with existing post link>, reply_to=<msg id>)`; if `fixed in` filled, also template C + retag + close on that post |
| `close` | existing post: `reply_post(post, <template C>)` if `fixed in` filled → `add_tags(post, ["done"])` → `close_post(post)` |
| `retag` | `add_tags` / `remove_tags` per `note` (e.g. archived post missing `done`); `add_tags` handles archived threads itself |
| `skip` | nothing |

Post link = `https://discord.com/channels/<guild>/<post id>` (guild from the message link).

**Discord copy is short, plain English** (USER 2026-09-18). Templates — fill, do not embellish:

- **A** (post body): `<summary sentence>\n\nReported in #general: <message link>`
- **B** (reply under #general message): `Tracked in #issues: <post link>`
- **C** (fixed reply in post): `Fixed in v<version>.` — desktop versions say `desktop v3.6.8`,
  mobile `mobile v3.6.8`, matching `changelog/` file names.

After the run: rewrite `triage.md` with a `result` column (post ID or error) and print the
counts. Write `state.json` `{"last_seen": "<newest message id of the scan>"}` — the next `scan`
starts after it, so replied, skipped and discussion messages are never re-read.
Then commit + push `state.json` (and `channels.json` if changed) straight to `main` without
asking — admin lane (USER 2026-09-19: 「apply完就直接commit,不需要問我」).

## Rules

- `done` is applied only by `close` / `fixed`; `drop` is never applied by the skill.
- `fixed in` comes from `changelog/` (released) or a MERGED PR on `main` (unreleased → next
   train version, proposed) — never from an open PR or an unmerged branch.
- A "fixed" / `close` row the USER did not confirm is never applied; `apply` only runs rows the
   USER left in the file. Release scope is the USER's call.
