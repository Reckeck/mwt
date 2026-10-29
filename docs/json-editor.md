# Vault JSON editor

The "Settings → Open vault → View JSON" screen (dev build only,
`import.meta.env.DEV`). It shows the decrypted vault document in CodeMirror and
lets you edit it directly — with field hints, schema validation and
autocompletion from Rust.

## What lives where

| Layer | File | Responsibility |
|---|---|---|
| Schema | `crates/core/src/model/vault.rs` → `json_schema()` | the single source of the document shape (`schemars` over `VaultDoc`) |
| IPC | `crates/app/src/commands/settings.rs` | `vault_schema`, `dev_dump_vault`, `dev_apply_vault_json`, `dev_open_vault_json_in_editor`, `dev_reload_vault_json_from_editor` |
| Autocompletion | `src/shared/lib/json-schema-reader.ts` | `$ref` / `anyOf` / `oneOf` / `type: [...]` → flat suggestions |
| Editor | `src/shared/lib/editor/` (`theme`, `json-path`, `completions`, `extensions`) | CodeMirror extensions: theme, autocomplete by JSON path, lint, history, folding |
| JSON helpers | `src/shared/lib/json.ts` | `parseJson` (whether the draft parses) and `normalizeJson` (canonical form for comparison) |
| Component | `src/widgets/vault-json-editor/VaultJsonEditor.vue` | glue: CodeMirror + IPC + status |

The document shape exists nowhere in the frontend: the editor asks the backend for the schema.
Add a field to `VaultDoc` and it shows up in the suggestions by itself.

## Editing model

**A draft does not write to the vault.** Saving is Ctrl+S only. The rest:

- typing only re-validates the draft (400 ms debounce) and moves the status badge;
- a draft that does not parse is **not saved at all** — the last saved state
  stays on disk, and the line under the editor explains why;
- "Reset" returns the buffer to what the backend returned after the last
  successful save.

The backend also rejects a document before writing: invalid JSON, an unknown schema
or a wrong payload version → `Error`, and the `.bak` copy is untouched. The only
place a backup is created is `dev_apply_vault_json`, and only after a successful
`decode`.

## Status badge

| Status | When |
|---|---|
| `loading` | the schema and document are loading |
| `saved` | the draft is canonically equal to what is on disk |
| `pending` | the draft is valid and differs from disk |
| `applying` | a write is in progress |
| `invalid` | the draft does not parse (the message is inline, next to the field) |
| `rejected` | the backend rejected the document (details in a toast) |

## Autocompletion

The path is taken from Lezer's own syntax tree, so it is correct by
construction — there is no string scanner in the code. The schema is resolved through `$ref`,
`Option<T>` expands to an `anyOf` with `null`, and an `enum` can arrive as a flat
list or as a `oneOf` of `const`.

- `Ctrl+Space` or a 1.5 s pause after the last keystroke — open the list;
- `↑` `↓` — select, `Enter` — insert, `Tab` — indent, `Esc` — close.

The list is not reopened if it is already open: that would move it under the
caret, which the user has already moved.

## External editor

"Open in editor" writes the document to a temporary file and launches `$EDITOR`
detached (the command returns immediately, the window is not blocked). "Pull from
editor" reads **only** that file — an arbitrary path cannot be requested from the
frontend, and the command does not accept one.

## Errors and notifications

A syntax error stays inline: it is tied to the caret and must be readable next to the draft.
Everything else is a toast (`vue-sonner`): a successful
write, a backend refusal, schema/document load errors, external
editor operations. Inline and toast are not duplicated for the same event.

## Schema after Phase 2

The schema is built from `VaultDoc` (`vault_schema`), so the new sections —
`contacts`, `categorization_rules`, `manual_prices`, as well as the still empty
`automations`, `automation_runs`, `budgets`, `goals`, `reminders`,
`price_alerts`, `utility_items`, `meter_readings`, `enrichments` — appeared in
autocompletion by themselves. `categories` is now a list of objects
`{ key, name, group, archived }`, and transactions are eight variants with an external
tag (`{ "topup": { … } }`).

A schema error on "Apply" is shown by the editor as a parse error
(`DESERIALIZE`), not as `INCOMPATIBLE_VAULT`: it is a typo in the draft, not an
old file.

## What is dangerous here

The file contains **all** transactions and Bybit keys in plain text. Do not commit
it, do not push it to a repository, do not show it in a screenshot without preparation.
