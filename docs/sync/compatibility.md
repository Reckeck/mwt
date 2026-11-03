# Compatibility between versions

Part of the sync documentation, see [the overview](../sync.md). Previous:
[The external copy](external-copy.md). Next:
[Troubleshooting](troubleshooting.md).

Devices are updated one at a time, so sync must behave when the app versions on
the two sides differ. The policy is binding for every sync change.

## How a device learns the other's version

Every session starts with a `Hello` from each side (the dialer and the listener
alike). It carries:

- `protocol`: `PROTOCOL_VERSION`, now 2 (2 added the `security` notice category);
- `format`: `VAULT_FORMAT_VERSION`, now 3;
- `app_version`: the sender's app version, informational only.

The checks, in order: the vault proof (a different vault is `SYNC_OTHER_VAULT`),
then `protocol` and `format` (a mismatch is `SYNC_VERSION_MISMATCH`), then the
pause and forgotten checks (see [Pairing and trust](pairing.md)).

The wire format is MessagePack with field names. Consequences:

- an unknown field is ignored;
- a missing field without `#[serde(default)]` fails decoding;
- an unknown `Message` variant fails decoding.

## Same protocol and format, different build

The devices connect and exchange. Differences come only from features one side
lacks:

| Feature | New side | Old side |
|---|---|---|
| Decline pauses the device | closes the paused device quietly with `Bye "paused"` | keeps dialing with its normal backoff and sees no error |
| Cut-off notices | sends `Bye "paused"` or `"forgotten"` | sees a plain `Bye` and closes quietly |
| Timeout is not a decline | sends `Bye "timeout"` | treats any `Bye` as a quiet close |
| Secrets hidden in labels | hides them in its own window and journal | shows and journals secrets received from the new side, because the receiver builds the labels |
| Price and rate thinning | thins after every apply and stays within the caps | re-sends thinned points each session (extra traffic; the new side re-thins and converges) |
| External copy push | detects a blind overwrite and merges on open | still pushes blindly |
| Records during the question | holds them, asks again | applies them together with the answer |
| Dedupe keeps later edits | carries later field edits into the kept copy | keeps the creation-stamp rule and may drop an edited copy |
| Run keys | skips a booked key, merges duplicates by key or `planned_at` | may still run the same occasion twice |
| Marks per trigger | merges `marks` entry by entry, never drops an entry the old map lacks, sends a whole-map head for the old side | stamps and sends the whole map |
| Transaction cursor | device-local, ignores the incoming value | still sends and takes `tx_cursor` |
| Tombstones | compacts only when every gating device (forgotten ones for 365 days) has them in its manifest, drops an old compacted tombstone coming back | compacts by its own rule (180 days and its own sync times) |
| Peers during the question | merges them, re-checks trust before applying | drops `Peers` there |
| Log ties | breaks ties by key, same records everywhere | keeps insertion order, may keep other records at the cap |
| Reminder ids | `auto-<kind>-<entity>` | random ids; dedupe bridges both by source |

The new side keeps tombstones longer, so a record deleted on the new side does
not come back from an old peer that kept it. Nothing new goes on the wire:
`SyncState.tombstone_acks` is device-local and a missing entry means "keep".

A mixed pair converges: an old device that double-runs a job produces a
duplicate, the new side merges it after the next exchange (two exchanges at
most), and there is no delete-and-recreate loop. The exception is a copy edited
later on the old device: its older dedupe may delete the edited copy, so the
"What's new" entry tells the user to update the app on every device.

Local vault fields of a newer version (`SyncState.paused`, `cut_by`) are dropped
when an old build re-saves the same vault file (a downgrade, or the external copy
opened by an old device). The result is "no pause". That is accepted.

## Different protocol or format

Both sides refuse right after the proof. No records, peers or manifest are
exchanged and nothing is changed in either vault.

- Each side shows once: "The app versions are incompatible: <name> is on
  <version>, here it is <own>. Update the app on both devices." When the peer's
  version is empty (a build that does not send it): "... is on an old version".
  The `sync-error` event carries `peer_name`, `peer_version` and `own_version`.
- The notice is deduplicated per (code, device) and shows again only after a
  successful exchange with that device.
- Trust is kept, the device stays in the list, and the address is not counted as
  failing. Calls continue with the normal backoff, so the pair joins by itself
  once both are updated.
- The group splits by version: a client that cannot link to its main counts the
  main as lost and takes part in a new election. Old devices exchange among
  themselves and new ones among themselves.
- After the update, records merge per field by stamps, with no data lost. One
  exception: a deletion that an old build already compacted can come back from
  the other side until both are updated.

## Rules for every sync change

1. A new `Hello` or message field is `#[serde(default)]`, and its absence means
   the old behaviour.
2. A new `Message` variant, or a changed meaning of an existing one, either bumps
   `PROTOCOL_VERSION` or is sent only to a peer that announced support in
   `Hello`. A bump splits groups until every device is updated, so it needs the
   owner's decision.
3. A new vault field is additive with `#[serde(default)]`, or
   `VAULT_FORMAT_VERSION` is bumped **with a migration** (AGENTS.md rule 5). A
   format bump also stops sync with older builds, so "What's new" says to update
   every device.
4. Local bookkeeping (`doc.sync` except the records) never travels over sync and
   is not taken from the external copy.
5. Every change states what an old peer sees, what the new side does with it, and
   the test.

The fields added by the repair stages follow the rules: `Hello.app_version`,
`Hello.instance`, `Hello.unreached`, `SyncState.paused`, `SyncState.cut_by`,
`TxOrigin::Workflow.run_key` and `Settings.schedule_timezone`. The cut-off notice
reuses the existing `Bye { reason }` variant.

## Tests that pin it down

In `crates/core/src/sync/protocol.rs` and `crates/app/src/sync/tests.rs`:

- an older `Hello` without the newer fields decodes, with an empty version;
- an unknown message variant is an error, not a panic;
- a different protocol or format is refused on both sides once, the peer stays
  trusted and the address is not marked failing;
- marks from an old peer do not erase another trigger's mark, and a double run
  from an old peer converges (`crates/core/src/service/peer_sync/marks_tests.rs`,
  `run_key_tests.rs`).

## What an old peer still gets wrong

- It may drop an edited duplicate (the creation-stamp dedupe) and show secrets
  from labels the new side did not mask.
- It may run a scheduled job, a transaction trigger or a question a second time;
  the new side merges the result, but the old device may also post a duplicate
  that it never removes itself.
- It compacts tombstones by its own rule, so a deletion can come back from it.
- It re-sends thinned cache points every session.
- It applies records that arrive during a change question together with the
  answer.
- It does not understand "paused" or "forgotten" reasons, so it keeps knocking
  until it is updated or paired again.

The remedy is the same everywhere: update the app on every device.

Next: [Troubleshooting](troubleshooting.md).
