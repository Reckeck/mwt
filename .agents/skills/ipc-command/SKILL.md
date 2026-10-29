---
name: ipc-command
description: >
  The procedure for adding or changing a Tauri command (IPC) in trade-app, from
  the DTO in ipc_contract.rs to the call from the frontend. Use for tasks like
  "add a command" («добавь команду»), "return field X in the response" («верни
  поле X в ответе»), "change the contract" («измени контракт»), "call from the
  frontend" («вызови с фронта»), and when fixing gen-types or an out-of-sync
  ipc.gen.d.ts. Triggers: tauri command, invoke, IPC, ipc_contract,
  ipc.gen.d.ts, gen-types, ts-rs, generate_handler, DTO, новый эндпоинт (new
  endpoint), RefreshResult, require_vault, err_to_string, an error from the UI
  of the form "CODE: message".
---

# Adding/changing a Tauri command

The architectural boundary is hard: **`crates/core` knows nothing about
Tauri**, and `crates/app/src/commands/*` is a thin IPC wrapper that only
translates DTOs into calls of the core service. Never put domain logic into
`commands/`.

## Layers and who is responsible for what

| Layer | File | What is there |
|---|---|---|
| DTO | `crates/core/src/ipc_contract.rs` | command inputs/outputs, `#[derive(Serialize, Deserialize, ts_rs::TS)]` |
| Domain | `crates/core/src/service.rs` (`UnlockedVault`) | all the logic + unit tests |
| Container | `crates/core/src/storage/mod.rs` | `SnapshotFormat`, encode/decode, version migration |
| IPC | `crates/app/src/commands/<module>.rs` | the `#[tauri::command]` handler |
| Registration | `crates/app/src/lib.rs` | `tauri::generate_handler![…]` |
| TS types | `src/shared/api/ipc.gen.d.ts` | **generated**, do not edit by hand |
| Call | `src/shared/api/ipc.ts` → `call<T>(cmd, args)` | the only `invoke` point |
| Documentation | `docs/ipc.md` | command table, checklist |

## Step-by-step checklist

### 1. DTO in `crates/core/src/ipc_contract.rs`

Sections are separated by divider comments (`// -- auth ---…`). Put the new
type in its own section, not at the end of the file.

```rust
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ArchiveStoreInput {
    pub id: String,
    pub archived: bool,
}
```

Rules:
- **`#[derive(ts_rs::TS)]` is mandatory.** Without it the type does not reach
  `ipc.gen.d.ts` and the frontend does not get the type (silently, with no
  compile error).
- **`Default` is mandatory** if `ipc.gen.d.ts` contains optional fields: the
  generator requires it for `export_all_to`. It also makes
  `CachedPrice::default()` meaningful.
- Fields are `snake_case` (the serde default); the TS glue accounts for it.
- Do not expose internal models (`AssetRef`, `Store`, `Transaction`): if the
  frontend needs a domain type, make a DTO with the needed fields.
- `Amount` in TS is a `string` (custom impl in `amount.rs`), not a `number`.

### 2. Logic + test in `crates/core/src/service.rs`

A method on `UnlockedVault` returning `Result<_, crate::error::Error>`:

```rust
pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> { … }
```

The unit test goes in the same file (`#[cfg(test)] mod tests`). Core is tested
without Tauri: the vault is created in a temporary directory, any password.

### 3. Handler in `crates/app/src/commands/<module>.rs`

Signature template (do not deviate from it):

```rust
#[tauri::command]
pub async fn archive_store(
    input: ArchiveStoreInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.archive_store(&input.id, input.archived)
        .map_err(err_to_string)
}
```

Mandatory invariants:
- Return `Result<T, String>`. The string comes from
  `err_to_string(e)` = `format!("{}: {}", e.code(), e)`.
- `require_vault(&state)` for everything that touches the vault; otherwise
  `Error::RequiresUnlock`.
- Map a vault lock error (`lock_vault`) to `Error::Crypto("lock poisoned")`.
- Args arrive as `input: XInput` (a single object), not flat. This lets you add
  fields without breaking changes to the call.

### 4. Register in `crates/app/src/lib.rs`

Add it to `tauri::generate_handler![…]` **in the same group and with the same
divider comment** as the other commands of the module. A forgotten
`generate_handler!` is the most common mistake: the command compiles, but
`invoke` on the frontend fails with "command not found".

### 5. Regenerate the TS types

```bash
mise run gen-types
```

Check the diff of `src/shared/api/ipc.gen.d.ts`: the new type **must** appear.
No diff means no `#[derive(TS)]` or the DTO is not exported in
`src/bin/gen_types.rs`.

### 6. Call from the frontend

```ts
import { call } from '@/shared/api/ipc'

const overview = useQuery({
  queryKey: ['store', storeId.value],
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value }),
})
```

- `call<T>(cmd, args)` is the only wrapper over `invoke`. A direct `invoke` in
  a component is a mistake.
- Take the type `T` from `ipc.gen.d.ts` (global types, no import).
- After a mutation invalidate the affected query keys:
  `void queryClient.invalidateQueries({ queryKey: ['stores'] })`.
- Outside Tauri `call` goes to the mock inside `src/shared/api/ipc.ts`: add a
  branch there if you want to check the UI in the browser via `pnpm dev`.

#### Argument names: camelCase outside, snake_case inside `input`

This is the most common place for mistakes. **Two different** rules apply:

```ts
// 1. Scalar command parameters → Tauri v2 expects camelCase
call<StoreOverview>('get_store_overview', { storeId: storeId.value })
//                                        ^^^^^^^^ Rust: store_id: String

// 2. DTO struct → passed as `input`, INSIDE the keys are snake_case
call<GetCandlesOutput>('get_candles', {
  input: { symbol: 'BTC', interval_min: 60, limit: 300 },
})
//                        ^^^^^^^^^^^^ serde: snake_case, camelCase does NOT work
```

That is: `#[tauri::command] fn f(store_id: String)` → `{ storeId }`, and
`fn f(input: FInput)` → `{ input: { …snake_case… } }`. The reason: camelCase is
the Tauri wrapper's convention for **function parameters**, whereas inside a
struct plain serde applies, and it is snake_case here.

- `Amount` comes from Rust as a **string**: do not parse it into a `number`
  for arithmetic; format it via `formatMoney` from `@/shared/lib/format`.

### 7. Update `docs/ipc.md`

Add a row to the table of the right module: `| command | input | output |`.
Keep the section "Adding a new command (checklist)" up to date.

### 8. Checks

```bash
mise run check
```

Required: `mise run test` (core), `mise run typecheck` (frontend),
`mise run lint`, and the diff of `ipc.gen.d.ts`.

## Iron rule: a `MutexGuard` must not be held across `.await`

`AppState.vault` is `Mutex<Option<UnlockedVault>>`. The guard is **not
`Send`**, so an `async fn` that holds the guard during `.await` will not
compile.

The correct pipeline (reference: `refresh_all` in `commands/pricing.rs`):

```rust
let data = {
    let mut g = require_vault(&state).map_err(err_to_string)?;   // 1. take
    let v = g.as_mut().unwrap();
    /* copy what is needed into local variables */
    (copied_data, base)
};

let fresh = state.pricing.fetch_prices(&copied_data).await?;      // 2. await without guard

let mut g = require_vault(&state).map_err(err_to_string)?;        // 3. take again
let v = g.as_mut().unwrap();
/* apply */
```

Three steps: **take → release → await → take again**. The compiler catches a
violation itself, but if you first write `.await` inside a block with the guard
you get a confusing borrowing error rather than a clear one about the mutex.

## Error codes

`Error::code()` in `crates/core/src/error.rs` returns a string that becomes the
prefix of the message. The frontend parses it in `parseError`
(`src/shared/api/ipc.ts`) and looks for the key `errors.<CODE>` in the locales.

A new `Error` variant → add its code to `Error::code()` **and** the key
`errors.<CODE>` to `src/shared/config/i18n/locales/ru.json` and `en.json`.
Otherwise the UI shows `UNKNOWN` or an empty string.

Existing codes: `SECRET`, `ALREADY_EXISTS`, `NOT_FOUND`, `UNSUPPORTED`,
`SERIALIZE`, `DESERIALIZE`, `CRYPTO`, `IO`, `NETWORK`, `INVALID_INPUT`,
`PRICING`, `MISSING`, `ARCHIVED`, `REQUIRES_UNLOCK`, wallet tree rules:
`STORE_CYCLE`, `STORE_TOO_DEEP`, `STORE_PARENT_ARCHIVED`,
`STORE_NAME_TAKEN`, `STORE_ROOT_MOVE`, `STORE_RESTORE_PARENT_FIRST`; old vault:
`INCOMPATIBLE_VAULT`.

## Common pitfalls

| Symptom | Cause |
|---|---|
| `invoke` → "command … not found" | not added to `generate_handler!` |
| An argument arrives empty/default | camelCase/snake_case mixed up (see §6) |
| No TS type, no errors | no `#[derive(TS)]` / no `Default` |
| `failed to serialize` / undefined on the frontend | the type exists in Rust, but the new field has no `#[serde(default)]` |
| The form is not cleared after saving | `invalidateQueries` forgotten |
| The error is localized as `UNKNOWN` | no `errors.<CODE>` key in the locales |
| `MutexGuard` in `.await` does not compile | the three-step rule above was violated |
| The list is empty in the browser (`pnpm dev`) | the mock in `src/shared/api/ipc.ts` has no branch for the new command |
