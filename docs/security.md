# Security

## Threat model (brief)

- The master password is neither stored nor transmitted; the vault file can be stolen —
  without the password it is unreadable.
- A wrong or guessed password yields a cryptographic failure —
  there is no verifier that would let offline guessing be queued
  or compared partially.
- Any change to the file (even a trivial one) invalidates the AEAD tag.

## File format

```
+--------+-----------------------------+------------------------+
| magic  | header  (55 B, = AAD)       | ciphertext + tag (16) |
| 8 B    |                             |                        |
+--------+-----------------------------+------------------------+
magic  = b"TRDWVLT\0"
header = u32 version | u8 kdf_id | u32 mem_kib | u32 iters | u8 par
       | [u8;16] salt | [u8;24] nonce | u8 payload_fmt
```

- **KDF** — Argon2id v0x13, 32-byte output, at the cost **stored in the header**:
  a file always opens with the `mem_kib`/`iters`/`par` it was sealed with, and
  every later save keeps them (`SealKey` carries the cost). `kdf_id`:
  `0` = the standard cost every vault below the crypto level uses
  (`m=65536 KiB (64 MiB)`, `t=3`, `p=1`); `1` = no password (plain payload
  with a SHA-256 checksum, the cost fields must be zero); `2` = the crypto
  level, a hardened cost picked on the device (`KdfProfile`: desktop 1 GiB,
  `t=3`, `p=4`; phone the heaviest of 256/128/64 MiB, `t=3`, `p=1`, that
  derives within 3 s). Builds before id `2` answer `Unsupported` ("update the
  app"), never a wrong password.
- **Bounds** — the header is read before anything authenticates it, so the
  cost is checked before a single block is allocated: `mem_kib` 64 MiB…1 GiB,
  `iters` 1…4, `par` 1…4 (`KdfParams::check`). Outside them is `Error::Secret`:
  a changed header is one more file that does not open.
- **Memory ceiling** — inside the bounds a header can still ask for more than
  the device has, and on Linux/Android an allocation that size "succeeds"
  under overcommit and gets the app killed. Every derivation (open, raise,
  calibration) reads `/proc/meminfo` at that moment and allows 3/4 of
  `MemAvailable` (`kdf_profile::kdf_memory_ceiling_kib`), clamped to
  64 MiB..1 GiB; without that file (Windows, macOS) it is 1 GiB.
  `set_kdf_memory_ceiling_kib` is an explicit override (tests); the
  ceiling is the lower of the two. A vault above it is `Error::VaultTooHeavy`
  (`VAULT_TOO_HEAVY`) before any allocation: the file is fine, it opens once
  other apps free memory or on a device with more.
  Calibration never tries a rung above the ceiling.
- **A cost change needs no container bump**: the cost is data in the header,
  not a constant; `VAULT_CONTAINER_VERSION` changes only with the byte layout.
- **Raising the cost** (`VaultFile::reencrypt_verified`) writes the new file
  to `.<name>.reencrypt.<pid>.<uuid>.tmp` (`create_new`), reads it back
  through the same handle, opens and compares it with the open document, and
  renames it over the vault only while the name still is that file with those
  bytes. A leftover of a crash is swept when the vault opens and when it is
  deleted. An empty password is refused.
- **Changing or setting the password** (`change_password_with`,
  `set_password_with` in `service/password.rs`). A change keeps the vault's
  KDF cost (`SealKey::with_new_password`: new salt, same id and parameters, so
  level 3 stays level 3). Setting a password on a plain vault takes the
  standard cost. The steps run in this order, and a crash after any of them
  leaves files that one of the two passwords opens:
  1. `<vault>.secrets` is sealed under the new key into `.secrets.next`; the
     file under the old key stays.
  2. The vault is written under the new key (atomic write; the mirror gets
     the same bytes in that write) and read back from the disk with the new
     key.
  3. `.secrets.next` is renamed over `.secrets`. If the process died before
     this rename, the next load with the new key does it.
  4. The copies are handled. Each re-sealed copy goes to a
     `.reencrypt.*.tmp` file, which is read back and opened with the new key
     before it is renamed over the copy.
  `vault_io::copies::inventory` lists every `<vault>.*.bak` (slot,
  `.vN.bak`, pre-sync, stamped) as plain, under the session key, under
  another key, or unreadable. It compares the header's salt and cost with
  the session key and runs no KDF. Pre-sync copies always follow the new
  key; on a change, one that cannot be re-sealed is deleted. Plain copies are
  kept, re-sealed or deleted as the caller asks. A copy is deleted only
  after step 2 succeeds. Copies under the old password are re-sealed only
  on request, using the old password once per salt. Copies the new password
  does not open are returned in the report for the user to choose. Removing
  the password is refused while exchange keys are on this device: their
  file would be sealed under a key anyone can derive.
- **AEAD** — XChaCha20-Poly1305: `key = XChaCha20Poly1305::new(derive_key(pw, salt))`,
  `nonce = random 24B`, `aad = header.bytes()`, a new random salt+nonce on
  every write (no nonce reuse across collisions).
- **payload_fmt** — `0` = JSON (dev), `1` = bincode (release before v2, read-only), `2` = MessagePack with field names (release since v2); the file is self-contained.
- **Atomic write** — a temporary file next to it (`.<name>.tmp.<pid>.<uuid>`), write,
  `fsync(file)`, `rename`, best-effort `fsync(dir)`; on error the tmp file is removed.

## Key invariants

1. The KDF cost is the header's, not a global; `KDF_MEM_KIB/KDF_ITERS/KDF_PAR`
   are only the standard cost (`kdf_id` 0) new vaults get.
2. `VaultHeader::from_bytes` checks the length and that `kdf_id` is 0, 1 or 2;
   `KdfParams::check` checks the cost bounds before allocating.
3. All integrity rests on AEAD: there is no separate MAC/checksum (and none is needed).
4. The key is derived by Argon2 once at unlock and lives in memory while the
   vault is open (`crypto::SealKey`, `Zeroizing`, wiped on lock and
   exit). Every save encrypts with it using the same salt and a fresh random
   24-byte nonce (XChaCha20 allows this); a new salt only on
   vault creation and password change. Previously Argon2 (64 MiB) ran on every
   save: 80-230 ms under the vault lock on a PC, several times longer on a phone.
   The password was already kept in session memory (`Zeroizing<String>`), so
   a key next to it adds no attack surface. The key's `Debug` does not
   print its bytes.
5. Tests cover: wrong password, tampered file, missing magic,
   salt/nonce randomness (in fact — that they are random).

## Security levels

The level is not stored: it is the vault container's `kdf_id` (0 = level 2,
PIN; 1 = level 1, plain; 2 = level 3, crypto). Code:
`crates/core/src/service/security_level.rs`, the rule is mirrored in
`src/shared/lib/vault-password.ts`.

- **A new secret.** Level 2: a PIN of at least 6 digits, digits only
  (`PIN_FORMAT`). Level 3: at least 16 characters and not weak by an own
  heuristic without a dictionary (few distinct characters or mostly runs like
  `aaaa`, `1234` is weak; `PASSWORD_TOO_WEAK`). An older shorter password still
  opens its vault and is accepted where the current one is typed.
- **The raise to level 3** (`raise_to_crypto_level`) checks the current PIN
  first (a wrong one is `SECRET`, nothing changes); a level-1 vault has none, goes
  straight to level 3 and its current password is ignored. Its steps leave, if the process dies after any of
  them, a vault that opens at exactly one level with all its data: a backup of
  the file as it is (level 2, or a plain copy at level 1); a re-encryption at the hardened cost into a temp file that
  is read back, opened and compared before the rename over the vault (before
  it the vault is at its old level, after it level 3); the session takes the new key and
  the device secrets follow it; the document is written again (the external
  copy gets the same bytes) and read back from the disk; only then the copies
  cheaper than level 3 next to the vault (the backup, `.bak`, `.vN.bak`, the
  pre-sync ones) are re-sealed under the new key or, if they cannot be,
  deleted. A plain copy never stays next to a level-3 vault after a raise. After an
  interruption the backup stays and opens with the old PIN (or is the plain copy).
  The KDF cost is calibrated on this device, or on the phone profile when the
  vault has an external copy (it may be opened on a phone). Progress goes to
  the `security-raise-progress` event.
- **No downgrade.** A vault that holds crypto data (stage 1: exchange keys on
  this device or their file next to the vault) does not go below level 3:
  removing or weakening its password is `SECURITY_DOWNGRADE_REFUSED`, and the
  external copy is not pulled in when its bytes are protected more cheaply
  than the local vault (the link is marked failed instead). A password change
  keeps the vault's KDF cost.
- **Sync.** A batch with crypto data is refused below level 3 with
  `SECURITY_LEVEL_REQUIRED`; the refusing device tells the other one, which
  shows the same code with the device's name. Ordinary records sync at any
  level.
- **PIN delay.** On a level-2 vault five wrong PINs are free, then each next
  try waits 30 s, 1 min, 5 min, then 15 min (`PIN_RETRY_LATER`). The counter is
  kept per vault path in the app settings, so it survives a restart, and is
  reset by a correct PIN. It only slows guessing through the app: a copied
  file is guessed offline at the KDF's cost, which is what level 3 is for.
- **Heavy vault.** A vault whose KDF needs more memory than the device can give
  is refused at unlock with `VAULT_TOO_HEAVY` instead of crashing.

## Window Content Security Policy

The app window runs under a strict CSP (`app.security.csp` in
`crates/app/tauri.conf.json`): if foreign code gets into the interface (an XSS
in a library, text from an import), the browser will not run it and will not let
it send data out.

- `script-src 'self'` — only the build's scripts. The theme inline script in
  `index.html` is allowed by Tauri itself, by a hash at build time; `eval` and
  `new Function` are forbidden.
- `style-src 'self' 'unsafe-inline'` — inline styles are needed by Vue, Reka and ECharts.
  `dangerousDisableAssetCspModification: ["style-src"]` stops Tauri from appending
  hashes here: with them the browser would stop honoring `'unsafe-inline'`.
- `connect-src ipc: http://ipc.localhost` — the webview's network access is only to the backend.
  Prices, FX, exchanges and sync go to the network from Rust; a new external request
  from the frontend also goes through a command, not by widening the CSP.
- `img-src 'self' data: blob:`; `object-src`, `base-uri`, `form-action` — `'none'`.
- `devCsp` for `mise run dev` adds only Vite HMR (`ws:`,
  `http://localhost:1420`) and the dev server's inline scripts.

Checked 2026-11-03: all check-layout screens and overlays under this policy
in Chromium and the Tauri window on Linux (WebKitGTK) — no violations, an injected
inline script is blocked.

## Recommendations for the user

- Level 3 with a password of 16+ characters, unique, not from a dictionary, for a vault
  with crypto data. Everything else is already covered by the KDF parameters.
- Back up the vault file (it is just a file, safe to copy).

## Known limitations / hardening roadmap

- The PIN delay (see "Security levels") limits guessing through the app only; it
  does not protect a copied file.
- The decrypted vault key sits in the memory of the `multi-wallet-tracker` process; YubiKey/TPM
  key isolation is out of scope for v1.
- `categories` and prices are not encrypted separately (the whole document body is encrypted as one,
  so this is not a problem).

## Device sync

Exchange happens inside a Noise channel after proof that the same vault is
open on both sides; the first pairing uses the same code on two screens.
The threat model and protections — `docs/sync.md`.

Transferring a vault to a new device hands over an encrypted file: whoever receives
it can guess the password offline. So the file is sent only after the
device is chosen, the password is re-entered on the sender, "Accept" is pressed on the receiver
and a one-time code verified by SPAKE2 is entered; each transfer is visible in the history and
arrives as a notification on all linked devices (`docs/sync.md`,
"Vault transfer").

## Notifications while the vault is locked

While the vault is open, the scheduler once a minute (and on lock) takes a
`NotificationSnapshot`: reminder dates for 60 days ahead, price alert thresholds with the
latest rates, pending automations. It is kept in memory and on disk —
`<app_config_dir>/notify-snapshot.bin`, **only encrypted** with
XChaCha20-Poly1305 using a random 32-byte key that lives in the system
keyring (Secret Service / Keychain / Credential Manager, the `keyring` crate).
Without a keyring the file is not written at all: the schedule lives in memory until the
app exits. The snapshot is never stored in plain text.

What the snapshot reveals to someone who obtains both the file and access to the user's
keyring: reminder texts and alert thresholds for 60 days ahead — but not
balances, transactions, exchange keys or the password. The "Hide details
while the vault is locked" option replaces notification text with a neutral phrase.

Whatever was shown while the vault was locked is carried into the vault
journal on the next unlock (`apply_locked_deliveries`), and fired alerts are marked — with no
repeats.
