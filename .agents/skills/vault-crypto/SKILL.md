---
name: vault-crypto
description: >
  Invariants of the encrypted vault in trade-app: container format, versions,
  VaultDoc migrations, Argon2id/XChaCha20-Poly1305, atomic writes, money
  precision via Amount (Decimal) and the rules for handling secrets. Use when
  changing model/vault.rs, storage/mod.rs, crypto/*, amount.rs, service.rs, and
  for tasks like "add a field to the vault" («добавь поле в vault»),
  "migration" («миграция»), "old file does not open" («старый файл не
  открывается»), "wrong password" («неверный пароль»), "data loss" («потеря
  данных»), "amount precision" («точность сумм»).
  Triggers: vault, VaultDoc, VAULT_FORMAT_VERSION, VAULT_CONTAINER_VERSION,
  SnapshotFormat, bincode, atomic_write, Argon2id, XChaCha20,
  header, AAD, Amount, Decimal, f64, key.zeroize, секрет (secret), пароль
  (password), миграция схемы (schema migration).
---

# Vault: format, crypto, money

This is the riskiest layer of the project. Here a mistake costs the user their
data, not just a build. Every change must come with a test.

## Two independent versions: do not confuse them

| Constant | What it versions | When to bump |
|---|---|---|
| `VAULT_CONTAINER_VERSION` (currently `1`) | the envelope: magic, header fields, KDF parameters, cipher | only when the byte layout of the container changes; otherwise old vaults stop opening |
| `VAULT_FORMAT_VERSION` (currently `2`) | the `VaultDoc` payload schema, the `doc.version` field | only for an incompatible schema change and **only together with a migration** |

Both live in `crates/core/src/lib.rs`. File format:

```
[ VAULT_MAGIC 8B = "TRDWVLT\0" ][ header 55B ][ ciphertext+tag 16B+ ]
                        ^^^^^^^^^
                     used as AAD
```

`payload_fmt` in the header: `FMT_JSON`(0) is debug; `FMT_BIN`(1) is bincode
from release builds before v2, **read-only**; `FMT_PACK`(2) is MessagePack with
field names, release since v2. The file is self-describing: the application
reads all three.

## Backward compatibility (mandatory since 2026-10-11)

The user has a real vault. Every existing file must open and keep its data
after an update.

### Adding a field (the most common operation)

1. Add the field with `#[serde(default)]` (and `#[ts(optional)]` if the
   frontend mocks would break otherwise). JSON and MessagePack store field
   names, so an old v2 file will still be read; **no bump needed**.
2. Never use `skip_serializing_if` on persisted fields.
3. Test: a document without the new field is read in `Json` and `Pack`
   (example: `a_v2_pack_payload_tolerates_a_missing_defaulted_field`), and
   `fixtures::full_doc()` fills the new field with a non-empty value; its
   round trip is checked by `a_full_document_round_trips_in_every_format`.

### Incompatible change: bump + migration

Renaming a field, changing its type or meaning, removing an enum variant:

1. Copy the current shape of the changed types (and their parents up to
   `VaultDoc`) into a new module `storage/vN.rs` (example: `storage/v1.rs`).
   These definitions are never edited again.
2. `impl From<VaultDocVN> for VaultDoc` (or a `vN → vN+1` step), losing
   nothing.
3. `impl storage::chain::Legacy` for the frozen document (its `VERSION` and
   the schema one step up), add one line to `schema_chain!` in
   `storage/chain.rs` and bump `VAULT_FORMAT_VERSION`. There is no `match` to
   edit: `decode_versioned` is generated from that listing.
4. Tests: one `migration_cases!` block in `storage/mod.rs` with the payload
   of that version and the formats a file of that era could hold — JSON
   **and** the binary format. It generates one test per format: the document
   migrates without loss, opening creates `.vN.bak`, the file is rewritten at
   the current version and is migrated only once (example: the v1 block, with
   `Json` and `Bin`).
5. Update `docs/data-model.md` ("Versions and migrations").

A bump without a migration is not allowed: `decode` rejects an unknown
version, and the user loses access to their data.

## Crypto: what you must not do

The primitives are defined in `crypto/mod.rs` and **must not be changed**
without changing `VAULT_CONTAINER_VERSION` (which would break every existing
vault):

- KDF: `Argon2id`, v0x13. The **cost** is not a primitive: it is stored in
  the header and a file opens with its own (`header.kdf()`, `SealKey`
  carries it into every save), so a new cost needs **no container bump**.
  `kdf_id` 0 is the standard cost (64 MiB, 3 iterations, parallelism 1:
  `KDF_MEM_KIB`, `KDF_ITERS`, `KDF_PAR`), 1 is no password, 2 is the crypto
  level, hardened per device (`crypto/kdf_profile.rs`: `KdfProfile`,
  `calibrate`). Old builds answer `Unsupported` to id 2.
- KDF bounds: `KdfParams::check` (64 MiB…1 GiB, 1…4 iterations, 1…4 lanes)
  runs before any allocation; outside them is `Error::Secret`. Do not widen
  them past what a profile writes: a changed header could hang or kill the
  unlock.
- Memory ceiling: `kdf_profile::kdf_memory_ceiling_kib`, read at derive
  time: 3/4 of `MemAvailable` from `/proc/meminfo` (1 GiB where there is no
  such file), lowered by the `set_kdf_memory_ceiling_kib` override. A cost
  above it is `Error::VaultTooHeavy` (`VAULT_TOO_HEAVY`), never `Secret`: the
  file is fine, the device is too small. `try_reserve` alone is no guard
  under overcommit. Calibration never tries a rung above the ceiling.
- Only a `SealKey` writes a vault outside tests: `VaultFile::create`/`save`
  and `crypto::random_header` seal at the standard cost and are
  `#[cfg(test)]`, so nothing can quietly lower a hardened vault.
- Cipher: `XChaCha20-Poly1305`, 24-byte nonce, 16-byte tag.
- Key: 32 bytes, `key.zeroize()` after the cipher is created: **do not
  remove it**.

Invariants:

- **There is no password verifier in storage and there must not be.** A wrong
  password and a tampered file are indistinguishable by design: both give
  `Error::Secret`. This is a deliberate trade-off, not a bug. Do not "improve"
  it with a hash verifier.
- A decryption failure maps to `Error::Secret` (`map_err(|_| Error::Secret)`),
  not `Error::Crypto`; otherwise the UI will not offer to retry the password.
- **No `unwrap`/`expect`/panics on the decrypt path.** A mutated, truncated or
  foreign file is an `Error`, not a panic. This is covered by the test vector
  (see below).
- AAD = the header bytes. The header takes part in authentication: changing any
  byte of it (including `payload_fmt`) breaks tag verification.
- **Session key.** Argon2 runs only on open (`VaultFile::open_keyed`), on
  creation and on password change (`SealKey::fresh`, new salt). Saving uses
  `crypto::seal_with(&SealKey, …)`: same salt, new nonce. Do not bring the KDF
  back into the `persist` path: that would again cost 80-230 ms under the vault
  lock on every action.
- **Batched writes.** In the application `persist()` only marks changes; the
  `vault_writer` does the writing. Code that reads the vault **file** (copy,
  backup, transfer, external copy) calls `flush_pending()` first. A new way of
  closing the vault: `flush_pending()` first, and do not close on error.
  Details: `docs/storage-platforms.md`, "Batched writes".
- Writes go only through `atomic_write`: temp file + `fsync` + `rename`. A
  direct `fs::write` to the vault path is not allowed: a crash midway leaves a
  file that never opens again. The one other writer is
  `VaultFile::reencrypt_verified` (raising the cost): its own
  `create_new` temp file, read back and verified through the same handle,
  renamed only while the name is still that file.

`switch_format` (`service.rs`) exists to force a JSON↔MessagePack switch (a
request for bincode writes MessagePack). This is also a migration, so it needs
a round-trip test.

## Money: only `Amount` (Decimal)

```rust
pub struct Amount(pub Decimal);
```

- **Domain quantities, prices, PnL and fees are `Amount` only.** `f64` is
  forbidden in the domain: it cannot do 0.1 + 0.2 without artifacts, and an
  accumulated error in a portfolio is unacceptable.
- `f64` is allowed **only in `market/`**: OHLCV candles for charts are not
  money but points to draw. Conversion to and from the domain is explicit, at
  the boundary (`Amount::to_f64()` / `Amount::from_f64()`).
- API: `Amount::parse`, `from_f64`, `is_zero/is_negative/is_positive`, `abs`,
  `max`, `to_f64`, `round(dp)`, `into_inner`.
- Serialization is a **string**; in TypeScript `Amount` = `string`. The
  frontend does not parse it into a `number` for arithmetic.
- Rounding only via an explicit `round(dp)`. There must be no implicit
  rounding.

`Amount` is a newtype with its own ts-rs impl in `amount.rs`; adding another
representation breaks compatibility with saved files and locales.

## Secrets

- The password and the Bybit API key live **only inside the encrypted
  `VaultDoc`** and never leave core.
- Only a mask like `abcd…wxyz` is exposed (`ExchangeKeyStatus.hint`).
- Never log the password, key, salt, nonce or decrypted payload.
  `RUST_LOG=info` is enabled through `mise`, but nothing secret must leak into
  the log: check `tracing`/`println!` in core.
- Vault files and backups: `*.wallet`, `*.wallet.bak`, `data/` are in
  `.gitignore`. If you see them in `jj status`, do not commit them and tell the
  user.

## Test vector (mandatory)

These cases must return `Error::Secret` and **not panic**:

1. a valid file + a wrong password;
2. a file with a changed ciphertext byte (tamper);
3. a file with a changed header byte (including a KDF cost outside the
   bounds, or a kdf id changed to another known one).

These must return `Error::Unsupported` ("not a vault this build can read",
not a wrong password) and **not panic**:

4. a file truncated inside the header or the tag (cut later, inside the
   ciphertext, it is `Secret` like any tamper);
5. an empty file;
6. a file with a foreign magic;
7. a file with a `VAULT_CONTAINER_VERSION` or a kdf id from the future.

And a cost inside the bounds but above the memory ceiling is
`Error::VaultTooHeavy`, before anything is allocated.

Plus: vault creation, reopening, a round trip of all fields in all formats,
saving and reloading, migration from all previous versions.

All of these tests live in `#[cfg(test)] mod tests` next to the code
(`crypto/mod.rs`, `crypto/header.rs`, `storage/mod.rs`, `service.rs`).

## Checks

```bash
cargo fmt && mise run test        # core unit tests are mandatory
mise run check                    # full gate before committing
```

For a container change, additionally build the application properly:
`mise run build` checks that the format survives a real write cycle.

Do not forget `mise run gen-types` if you touched `Settings` or types that go
through IPC: they are exported to `src/shared/api/ipc.gen.d.ts`.
Documentation on the format is in `docs/data-model.md` and `docs/security.md`;
update them if you change the schema.
