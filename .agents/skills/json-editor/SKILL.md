---
name: json-editor
description: >
  Vault editor in JSON format (Settings → View JSON, dev-only): CodeMirror,
  document schema from Rust, JSON-path autocomplete, Ctrl+S, .bak before
  writing, $EDITOR. Use when editing crates/app/src/commands/settings.rs
  (vault_schema, dev_dump_vault, dev_apply_vault_json, dev_open_vault_json_in_editor,
  dev_reload_vault_json_from_editor), crates/core/src/model/vault.rs (json_schema),
  src/shared/lib/editor.ts, src/shared/lib/json.ts, src/shared/lib/json-schema-reader.ts,
  src/widgets/vault-json-editor/VaultJsonEditor.vue, and for tasks like
  "редактор vault", "JSON vault", "правка документа", "подсказки полей",
  "Ctrl+S", "схема vault", "autocomplete JSON", "VaultFile::backup",
  "отклонён документ", "черновик", "dev_apply_vault_json", "vault_schema",
  "Открыть в редакторе".
---

# Vault editor in JSON format

The vault document is shown and edited as is: the decrypted `VaultDoc` in
CodeMirror. The screen lives in `src/widgets/vault-json-editor/VaultJsonEditor.vue`,
the schema comes from Rust over IPC. The full description is in
`docs/json-editor.md`; here is only what you need to avoid breaking it.

## Three rules that are easy to break

1. **A draft does not write to the vault.** Only Ctrl+S (`dev_apply_vault_json`)
   does. Typing only re-checks the draft with a debounce and moves the status
   badge.
2. **A draft that does not parse is not saved.** Both on the frontend
   (`parseJson` before the call) and in the core (`storage::decode` before
   `replace_doc`) — two independent refusals. Do not bypass this "just in case".
3. **The shape of the document lives only in Rust.** The frontend does not know
   the vault fields — it asks `vault_schema` and builds hints from the answer.
   Add a field to `VaultDoc` and the hints update by themselves.

## Layers

| Layer | File |
|---|---|
| Schema (`schemars` over `VaultDoc`) | `crates/core/src/model/vault.rs` → `json_schema()` |
| IPC handlers | `crates/app/src/commands/settings.rs` |
| Backup before writing | `VaultFile::backup` (called only by `dev_apply_vault_json`) |
| `$ref` / `anyOf` / `oneOf` parsing | `src/shared/lib/json-schema-reader.ts` |
| CodeMirror extensions, theme, autocomplete | `src/shared/lib/editor.ts` |
| `parseJson` / `normalizeJson` | `src/shared/lib/json.ts` |
| Component + status badge | `src/widgets/vault-json-editor/VaultJsonEditor.vue` |

## What to change and where

**A new vault field** → `crates/core/src/model/vault.rs`. Nothing else: the
schema and hints pick it up by themselves. If you change only a DTO — `mise run
gen-types` and `mise run test`.

**A new hint in the list** → `src/shared/lib/json-schema-reader.ts`. The schema
from `schemars` arrives whole, including `$defs`; `$ref`, `Option<T>` as
`anyOf` with `null`, and `enum` both as a flat list and as a `oneOf` of
`const` — all four variants are already handled, do not duplicate them.

**Editor colors** → `HighlightStyle` in `src/shared/lib/editor.ts` on the
theme's CSS variables. Not `.cm-*` rules in CSS: without a registered
highlighter CodeMirror generates classes that the CSS will not hit, and the
document renders without color.

**The path for hints** → taken from `syntaxTree()` (Lezer), not from your own
string parsing. Do not rewrite it as a scanner: it is correct by construction.

**The "saved / not saved" balance** → comparison via `normalizeJson`, that is,
the canonical `JSON.stringify(JSON.parse(...))`. Whitespace and key order must
not count as an edit: the backend always returns its own formatting.

## Errors

Syntax — inline under the editor (tied to the caret, must be readable next to
it). Everything else — `toast` from `vue-sonner`: a successful write, a backend
refusal, errors loading the schema and the document, work with `$EDITOR`. One
event is either a toast or inline, not both.

Backend error codes (`DESERIALIZE`, `SERIALIZE`, `INVALID_INPUT`, …) arrive as
a prefix to the message; `errorText()` looks up `errors.<CODE>` in the locales
and falls back to the raw message.

## Pitfalls

| Symptom | Cause |
|---|---|
| Document without highlighting | the highlighter is set as `.cm-*` in CSS instead of `HighlightStyle` |
| Status forever "has unsaved changes" | comparison is on raw text, not through `normalizeJson` |
| "Synchronized" badge right after Ctrl+S | the baseline is updated from the sent text, not from the backend response |
| Caret jumps after saving | you do not save `hasFocus` and `selection.main.head` before `invalidateQueries()` — the dialog re-renders and takes focus |
| `invoke` → "command … not found" | the command is not in `generate_handler!` |
