# Device sync over the local network

Two or more devices with the same vault on one network find each other
and exchange changes directly, encrypted, without a cloud. The mode is
off by default (Settings → Sync). BLE is the next phase
on top of the same protocol; whole-file sync through an external copy
(`docs/storage-platforms.md`) remains a separate feature. These two paths
meet at sign-in: if the external copy changed on another device,
its records are merged into the open vault by the same path (`sync_manifest`,
`sync_records`, `sync_apply`, the "External copy" partner); the copy's `doc.sync`
is not taken, and the write to the copy is hash-checked and does not overwrite someone else's edits.

User decisions — 2026-10-17 (plan `bubbly-jumping-hare`).

## Where the code is

| Part | File |
|---|---|
| Change tracking, merge, undo (pure, no network) | `crates/core/src/sync/` — `stamp`, `state`, `registry`, `view`, `track`, `merge`, `plan`, `manifest` |
| Channel: Noise, pairing code, vault proof, clock | `crates/core/src/sync/session.rs`, `protocol.rs` |
| Vault: `persist` sets stamps, `sync_apply`/`sync_undo`/backup | `crates/core/src/service/peer_sync.rs` |
| Network: mDNS, TCP, roles, session | `crates/app/src/sync/` — `discovery`, `link`, `mod` (hub), `keystore`, `host` |
| Commands and events | `crates/app/src/commands/sync.rs`, `docs/ipc.md` |
| Interface | `src/features/vault-sync/`, `src/widgets/sync-settings/` |

## Change tracking

Vault mutations know nothing about sync. Every `persist()` compares the fields
of each record with the previous save (`sync::track::Tracker`, fingerprints in
memory) and puts a hybrid-clock **stamp** (`Stamp { wall_ms, counter,
device }`) on the changed fields. A missing record gets a **tombstone**. All
of this is stored in `VaultDoc.sync` (`SyncState`, an additive field — no format
bump needed):

- `stamps` — `collection:key` → `{ created, fields }`; a field stamp is stored
  only if the field was edited after the record was created;
- `tombstones` — deleted keys; those older than 180 days and confirmed by all
  devices are compacted; the age is counted from the later of the deletion
  stamp and the time it arrived here (`tombstones_seen`, local ms,
  not synced), so a deletion that arrives late is not compacted at once
  and the record does not resurrect from other devices;
- `peers` — trusted devices (key, name, revocation);
- `batches` — the last 20 batches (not older than 7 days) for undo.

Every `VaultDoc` field is described by a rule in `sync::registry`; a field without a rule
fails the `every_vault_field_has_a_sync_rule` test.

| Rule | Collections |
|---|---|
| records by key, field-level merge, deletions — tombstone | accounts, transactions (`meta` expanded by field), categories, contacts, planning, schemes and their queue/waits/state, exchange keys, vault settings (as one object) |
| log: union, ordered by time, trimmed to a limit, trimming is not deletion | `notification_log`, `automation_runs`, `workflow_runs`, `import_runs` |
| union of immutable points, no stamps | `price_cache`, `fx_cache` (first session — 90 days), `discarded_imports` |
| not synced | `holdings` (recomputed), `id`/`version`/`created_at`, `sync`, device fields in `settings` (`minimize_to_tray`, `fx_refresh_interval_sec`, `display_decimals` — now in `app_settings.json`) |

## Merge

For each field the larger stamp wins; equal stamps (data that was in
both copies before sync) are compared by value — the result is the same
on both devices, `A⊕B = B⊕A`. Edits to different fields of one record
are both kept. If one field was changed on two devices, the later
edit stays, and the batch summary says "Overwritten: was → became". A deletion
beats edits made before it; an edit after the deletion brings the record
back. A different transaction kind (the variant tag) — the whole record goes
by the latest stamp.

A batch is applied as one operation: records are assembled into JSON, the document
is re-parsed by serde — a record that does not fit the schema rejects the whole
batch. A record under a foreign key or from a non-synced collection is
`INVALID_INPUT`. After applying: duplicate transactions of one automation
or import (both sides managed to create them) are collapsed into one, and holdings are
recomputed. A workflow can create several transactions per planned moment
in one run (salary and tax): the earliest transaction wins, all transactions of its
run and all transactions of the same device stay; only transactions of another run
from another device are deleted. A transaction without a stamp is not deleted (neither an import
nor an automation). The copy that goes away hands the remaining one its later field
edits (with their stamps); the deletion and the field move are
written into the batch, so undoing the batch restores both copies.

## Confirm preview

The question about a list of changes carries, for each change, `fields`
(readable "field: before → after" lines) and `after` (capital and wallet
balances before → after), computed on a sandbox copy of the vault. Caches
(`Collection.cache`: `fx_cache`, `price_cache`) never ask, and points this
device would thin are neither offered nor requested (`sync_diff`).

## Undo

A batch stores each record "before" and the stamps "after". Undo restores the records
whose stamps have not changed since; those changed later stay as they are and
are listed. Undo is a **new local edit** with fresh stamps, and it
goes to the other devices: two copies of one vault cannot diverge
forever on one record. The emergency exit is `<vault>.presync.bak` (the last
three, written before the first batch of each session), "Restore from before the sync" in
settings.

## Network and roles

- mDNS `_mwt-sync._tcp`: the TXT holds only the tag `HMAC(vault.id, "mwt-disc" ‖ day)[..8]`,
  the main-device flag and the version. Outsiders see random bytes that change
  every day; the device name is not advertised.
- **Star.** If it does not see a main device for 3 seconds, a device becomes the main one;
  the others connect to it. With two main devices, the one with the larger
  random instance identifier yields. If the main device leaves, clients wait their turn
  to connect (rank) and the first becomes the main one. A device with "Make this
  device the main one" gets the role after its first exchange.
- **Port and the return call.** Sync listens on TCP 53318 (if busy —
  a random port). A client that could not connect to the main device does not
  become main itself: it waits, and after 6 s the main device calls every
  device of the group it sees in mDNS but is not connected to. After three
  failed attempts in a row the main device is considered gone. If a connection arrived from
  both sides at once, the one opened by the client stays.
- **Firewall.** The client reports in Hello (`unreached`) that it could not
  connect, and the main device shows "A firewall blocks incoming connections". The
  "Open the port" button adds a rule for the local
  subnet only: Linux — `pkexec` + ufw or firewalld (whichever is enabled), Windows —
  `netsh` through UAC, the Private profile. Flatpak is not allowed to touch the host firewall
  (without `--talk-name=org.freedesktop.Flatpak`) and shows commands for a
  terminal. On macOS the system asks by itself, Android has no firewall.
- **Diagnostics.** Settings → Sync: the port, the address, how many
  devices with this vault are visible, the last error; after 30 s without neighbors —
  a hint about what to check.
- **Only the main device** runs the scheduler's background tasks (automations,
  schemes, Bybit import, alerts, reminders); clients receive the result through
  sync. A manual price refresh works everywhere — caches are merged.
- **Notifications.** The system notification is shown by the device whose window is
  focused; if no window is focused, all of them. The bell updates everywhere.
  Notifications that arrived through sync and are older than an hour do not pop up.

## Session

1. Noise `XX_25519_ChaChaPoly_BLAKE2s`: both sides present their static
   device key.
2. `Hello`: the protocol and vault format versions, `HMAC(vault.id, handshake_hash,
   role)` — proof that the same vault is open (it does not reveal the id and is not
   reflected back), the time.
3. An unknown key — a "Pair" banner; when pressed, both screens show the same
   6-digit code derived from `handshake_hash`. Trust only after
   confirmation on **both** devices. A forgotten (revoked) key also goes through pairing: until the code is confirmed on both, it is not trusted and receives no data; a confirmed pairing brings it back (the newest record about the key decides).
4. The lists of trusted devices are merged (this is how "Forget" reaches everyone):
   per device the later record wins, but a revocation is lifted only by
   a local pairing, not by an incoming list; a record with a stamp beyond
   the allowed clock skew is skipped. A revocation that arrives closes the session with
   the forgotten device on the main device, and the others get the new list.
5. Manifests (`key → digest`) → a request for the differing records → records
   in chunks of 500.
6. The change list waits for the user after pairing, with "Ask before
   syncing" (only the first batch of a session) and from a "temporarily
   untrusted" device (every batch). Otherwise it is applied immediately, with a
   notification and "Undo".
   **"Decline" = a pause with this device.** The device id goes into
   `SyncState.paused` (in the vault, survives a restart and a lock; it is never
   synced and is not part of `peers`). While the pause stands, a session with it
   is quietly closed right after `Hello`: no error, no notification. In the device
   list — "Exchange paused" and "Resume" (`sync_resume_peer`):
   the pause is lifted, a connection is tried at once, and the change list will be asked about
   again with a fresh summary. "Forget" also lifts the pause. **A timeout is not a refusal:**
   no answer within `DECISION_TIMEOUT` (or the window is closed) — the session is quietly
   closed, no pause is set, the next session asks again.
   **A cut-off is named.** While a pause stands (and when the caller's key is forgotten), the device does not
   close the session silently but sends `Bye { reason: "paused" | "forgotten" }` (there are no new
   message variants: old builds see a plain `Bye`). A forgotten
   device gets `forgotten` if it did not ask for pairing (`PairRequest` right
   after `Hello`). The receiver writes `SyncState.cut_by[X] = { reason, at }` (locally, in the vault,
   not synced), shows `SYNC_PAUSED_BY_PEER` /
   `SYNC_FORGOTTEN_BY_PEER` once with the device name and stops dialing X (the main
   device's client, the main device in return calls, elections). A client that the
   main device refused does not become the main device itself: it waits for its call. X's own calls are accepted.
   The mark is lifted after a completed exchange, a pairing, "Pair again"
   (`sync_pair` with `deviceId`) and "Forget". A side that has a pause does not call the devices
   it paused either.
7. Live mode: saves are sent after 1 second of quiet, the main device
   forwards them to the rest; `Time` every 30 seconds, 95 seconds of silence is
   the end of the session.

## Threat model

| Threat | Protection |
|---|---|
| An outsider on the network reads or tampers with data | Noise (X25519 + ChaChaPoly), a tampered frame breaks the session (`SYNC_CHANNEL`) |
| An outsider learns that someone has a vault, or tracks a device | mDNS has only a daily HMAC tag, no name |
| A man in the middle (MITM) at first pairing | a 6-digit code from `handshake_hash` — the MITM's two screens show different codes; trust only after confirmation on both |
| A similar or foreign vault | proof by `vault.id` inside the channel (`SYNC_OTHER_VAULT`) |
| An old or new app with a different format | the protocol version and `VAULT_FORMAT_VERSION` (`SYNC_VERSION_MISMATCH`) |
| A lost device | "Forget device" propagates to everyone (`SYNC_REVOKED` when revoking on a live session); it can come back only by a new pairing with a code on both devices; the vault copy on it cannot be revoked — the advice is to change the password |
| A device with a wrong clock drags edits along | a skew of up to 5 min is normal; up to 24 h — a warning; more — a refusal (`SYNC_CLOCK_SKEW`); jumps during a session (drift > 2 min, time going back, a stamp from the future) — "temporarily untrusted": every batch waits for confirmation |
| An erroneous mass deletion | a backup before the session, undo of any batch within a week |
| Pairing brute force | 3 failures from an address — the address is ignored for 10 minutes; a frame is at most 16 MiB |
| Stealing a vault by transferring it to a foreign device | the device is chosen by name, a password on the sender, "Accept" on the receiver, a one-time code through SPAKE2 (one attempt per connection, 3 per request), a notification on all linked devices |
| Recipient substitution on the network | the key behind the address is checked against the fingerprint from mDNS; without the code from its screen the confirmation will not match |

The device key: on Android it is wrapped by a hardware Android Keystore key
(`crates/plugin-net`), on desktop it is in the OS password store (`keyring`). Without a
store the key lives in memory: after a restart you have to pair again
(the settings say so). The vault password is never transmitted: each
device has its own; after a password change the app reminds you that on the other
devices it is still the old one.

## Vault transfer

A new device receives the whole vault over the local network
(Settings → Sync → Vault transfer; on the unlock screen —
"Receive from another device"). Code: `trade_core::sync::transfer` (the protocol,
SPAKE2, file check), `crates/app/src/sync/transfer.rs` (sessions),
`sync/handover.rs` (mDNS, screens, saving).

1. The **receiver** advertises itself for up to 5 minutes in mDNS `_mwt-recv._tcp` (port
   53319): the TXT has only the device name, the type (`mobile`/`desktop`) and the fingerprint of
   its key (`sha256(pubkey)[..8]`). The name is as in LocalSend: an adjective
   and a fruit in the app language ("Juicy Orange"), permanent, in appconfig,
   the same as in sync; changed in settings.
2. The **sender** sees such devices, picks one and **enters the vault
   password once more** (checked in memory in constant time; a wrong one is
   `SECRET`, nothing is sent).
3. The channel is Noise XX, as in sync; the key behind the address must match the
   advertised fingerprint.
4. The receiver sees "<name> wants to transfer a vault" and **accepts or
   declines**. After "Accept" it shows a **6-digit one-time code**
   (CSPRNG).
5. The code is entered on the sender and verified with **SPAKE2** (symmetric,
   Ed25519). The PAKE key is bound to the Noise handshake hash, the confirmations
   (HMAC with the role) go in turn: the sender first. An eavesdropper has one
   attempt per connection and nothing to brute-force offline; after 3 wrong
   codes the request is reset (`SYNC_BAD_CODE`). A plain MAC of the code would not
   do here: an eavesdropper would crack it offline in fractions of a second.
6. The file is the encrypted `.wallet` as is (the password is not transmitted), in chunks of
   1 MiB with a SHA-256 of the whole; the receiver checks the size, the sum and that it is a
   vault container (`crypto::parse_file`), writes through `atomic_write` into the vaults
   folder under the original name (if taken — " (2)") and immediately offers to enter the
   password.
7. **Trail.** The sender writes the transfer into the sync history
   (`SyncState.transfers`, local) and a `security` category notification into the
   journal — it reaches all linked devices. The sender trusts the
   receiver immediately; the receiver does so after the first unlock
   of that file. Sync is turned on on the new device.
8. **Fresh trust.** A transfer is a new trust on both sides: it clears the
   pause, the cut-off and the revocation of that device, so a forgotten or
   declined device is linked again. The receiver keeps the pending trust in
   the app settings for 7 days, keyed by the file path, and applies it on
   the first unlock of that file. On that unlock `adopt_received_copy` resets
   the device-local bookkeeping the copy brought from the sender
   (`take_records_from` is the single list of what is taken from a copy and
   what stays local), so the new device starts with its own journal, undo
   batches and counters.
9. **Peer-list merge** is by stamp: a newer explicit trust lifts an older
   revocation, an older entry never lifts a newer one.

Port 53319 is included in the "Open the port" rule together with 53318. The sync
protocol was raised to 2: the notification log may now carry the `security`
category, which an old version cannot read — it gets
`SYNC_VERSION_MISMATCH`, not a parse error.

`Hello.app_version` (`#[serde(default)]`, empty for a build without the field) is the
sender's app version, only for the refusal text: the protocol does not change and
the version decides nothing. On `SYNC_VERSION_MISMATCH` the `sync-error` event
carries `peer_name`, `peer_version` and `own_version`, and the notification names the
device and both versions; an empty peer version reads as "an old version".

Secret fields. The `exchanges` and `attachment_keys` collections are hidden entirely
(`Collection.secret`); in other collections individual fields are marked in
`Collection.secret_fields` (`data_sources.url`, `workflow_vars.value`): in the
`before`/`after` of previews, batches and the journal they are shown as `•••`,
the record's signature does not contain them, and on open old previews of these fields
are masked too (idempotently). The other fields of the same record are visible.

## Limitations

- Sync runs while the vault is unlocked on both devices. On
  Android a minimized app continues it in a foreground service
  (`plugin-net`, type `connectedDevice`, a Wi-Fi lock, the notification permission
  is requested when sync is turned on): the notification shows the number of
  devices and the "Lock" and "Stop" buttons ("Stop" lasts until
  returning to the app). After the chosen time in the background without syncs
  (5-60 min, 30 by default, or never) the vault locks itself. An exemption
  from battery optimization is not requested: in Doze the connection freezes, and
  the next one transfers only what is missing.
- Clock skew is compensated through the hybrid clock: after a meeting the new
  edits of the lagging device sort after the edits of the one that ran ahead;
  edits made before the meeting are compared by their own stamps
  (the warning says so).
- Discovery is mDNS: on networks where multicast is forbidden (guest Wi-Fi with
  client isolation) devices do not see each other.
