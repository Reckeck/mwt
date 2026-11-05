# Compatibility between versions

Part of the sync documentation, see [the overview](../sync.md). Previous:
[The external copy](external-copy.md). Next:
[Troubleshooting](troubleshooting.md).

Devices are updated one at a time, so sync must behave when the app versions on
the two sides differ. The policy is binding for every sync change.

## How a device learns the other's version

Every session starts with a `Hello` from each side (the dialer and the listener
alike). It carries:

- `protocol`: `PROTOCOL_VERSION`, now 3 (2 added the `security` notice category, 3 made purchases entries of their own);
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
| Keep mark (`keep` on `TxMeta` and `Reminder`) | undoing a merge marks the copy it brings back; dedupe skips a pair holding a marked record, on every device with this build; «Allow merging» clears it | does not reach this build: the mark ships with protocol 3, so a protocol 2 device is refused at the handshake and cannot remove the copy or drop the mark. A vault that held an older build reads as `keep = false` |

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

## Lists inside a record, and the envelope journal

Between new devices an array field of a record (the purchases, people and
settlements of a shared event, the exchange keys, the envelope `moves`) merges
per entry by its id, so two devices adding to the same list both keep their
entry. A removed entry travels as the stamp of that entry.

With an old device the whole field is still one value:

- An old device's stamp of the whole field is a floor for every entry that has
  no stamp of its own. A head stamp equal to the newest entry stamp is what a
  new device adds for old receivers; it is derived and is no floor.
- An item deleted on an old device does not travel to new devices (an old
  device cannot say an entry is gone, so the new side keeps it).
- Envelope `allocated` is derived from `moves` between new devices: it is
  recomputed after every merge and stamped with the newest entry stamp, so a
  recomputation is never mistaken for an edit. An old device just writes
  `allocated`; a new device turns a write newer than what it holds into one
  correction entry `fix:<envelope id>:<stamp>` (its id comes only from the
  write, so it is made once however often it arrives). If the stamp of a
  correction equals the stamp of a real entry, the value was a new device's
  that an old one forwarded without the array stamp: the correction is
  withdrawn when the real entry arrives, and its stamp stays as the removal
  mark.
- The lines of a correction (`Adjustment`) split per asset: the entry key is
  the whole asset (`lines.<symbol>|<coingecko id>|<chain>|<contract>`), so two
  devices counting different assets of one correction both keep their value.
  `before` (the ledger balance just before the correction) is recomputed by
  every device from its own ledger. It stays in every stored and sent line (an
  old device requires it), but it never counts as a change: the change tracker,
  the merge and the sync plan compare a line without it
  (`Collection.recomputed`). A backdated operation arriving on one device
  therefore stamps nothing and cannot overwrite a balance the user counted on
  another. Whatever `before` a merged line carries, the recompute after the
  merge replaces it, so equal ledgers end with equal digests.
  An old device does not know the split: its recompute still stamps `lines` as
  a whole, and that stamp is a floor for every line without a stamp of its
  own, so it can win over a newer-looking edit exactly as before. The limit
  ends when every device is updated.
- The terminal of an exchange wallet (`exchange.terminal`: watchlist, active
  symbol, timeframe, indicators, drawings, layout) is device-local. A new
  device never stamps it, sends it, takes it from a peer or lets an undo touch
  it (`stores` has `skip: ["exchange.terminal"]`); the import settings beside it
  (`exchange.import`, `exchange.provider`, `exchange.import_interval_sec`) still
  sync. Limit with an old device: it still sends its terminal inside the store
  and stamps it, and a new device ignores both, so the old device's terminal
  never reaches a new one. The new device sends the store without a terminal,
  and an old device keeps a store as one whole record, so every store edit from
  a new device that wins there resets the old device's terminal to the default
  (a store received for the first time gets the default too). A store that
  reaches a new device from a peer is created with the default terminal.
- A car (`property.car`) splits one level further, per field
  (`property.car.mileage_km`, `property.car.osago_until`, ...), so a mileage
  update on one device and an insurance date on another both survive; a car
  that is `null` on one side follows the null-head rule of `settle_heads`. An
  old device stamps the whole car (`property.car`), and that stamp is a floor
  for every field without a stamp of its own, exactly as for the other split
  objects. `property.revalue` was already a field of its own.
- Manifest digests of the affected records change once after the update, so
  the devices re-exchange them once.

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

The locked-screen notification snapshot gained a `role_client` field. It lives
in the notify store outside the vault and is additive (`#[serde(default)]`): an
old snapshot reads as not a client and still fires.

## Shared events (stage M, catalog 116 and 123)

`Settlement.organizer` is a new optional field (`#[serde(default)]`, no format
bump). An old device ignores it on read and drops it when it resaves the
settlement through its typed document, so the organizer of a settlement is
lost. Rule R7b (the event organizer goes back to the one the settlements name)
then only flags the event: a settlement without the field is detected, not
acted on.

Guest contacts (`guest-…`) and payment links (`link-…`) now get deterministic
ids. An old device keeps writing random uuids for both. The duplicate pass folds
an old random guest contact into the deterministic one (the participant link and
the references to the contact move over), and rule R6 keeps one link per
operation (the earliest one) however the ids look.

## Protocol 3: shared-event purchases

`PROTOCOL_VERSION` goes from 2 to 3. Before, the purchases of a shared event
lived inside the entry `items.<id>`, which has one stamp, so the later write of
the whole item won and a purchase added on another device at the same time was
lost. Now every purchase is an entry of its own, `items.<item id>.purchases.<purchase id>`,
with its own stamp (`Collection.flatten_inner` in `sync/registry.rs`). Two
devices adding, editing or removing different purchases of one item all keep
their change; the purchases of an item are ordered the same way on every device
(the sequence both share, then the older change first, then by id).

- **2 and 3 do not sync.** A protocol 2 build would overwrite a whole item and
  lose or revive purchases, so the handshake refuses the pair. Nothing is
  exchanged and nothing changes in either vault; trust is kept and the pair joins
  by itself once both are updated.
- **What each side shows.** The protocol 3 side shows the usual message
  (`SYNC_VERSION_MISMATCH`): "The app versions do not match: <name> runs an old
  version, this device <own>. Update the app on both devices." (or, when the peer
  sent a version, with it). A protocol 2 build compares the hello the same way
  and shows its own existing text with the 3 side's version, "Update the app on
  both devices"; an old build cannot be changed.
- **Old vaults keep working.** No vault format change and no migration: stamps
  are additive keys. A purchase that has no stamp of its own takes the stamp of
  its item (`entry_stamp`), so stamps written before the split still say when a
  purchase last changed; the first edit of a purchase writes its own stamp.
  A device on protocol 3 sends the legacy `items` stamp too (`with_legacy_heads`).
- **A part that left with a payment comes back with it.** Deleting a payment in
  the ledger takes its purchases and settlements out of their events, and each
  stays in `SharedEvent.removed_with_payment`, merged by its own id. A newer edit
  on another device beats the deletion, so the payment can come back: the repair
  after the merge (and the local undo) then returns the part, unless the payment
  is linked elsewhere or the item is gone. A part removed by hand in the event
  leaves no record, so it never returns. The record goes once the payment is back
  or its tombstone is gone. An older build ignores the field and only misses the
  return.
- **A side without a purchase and without its stamp never saw it.** Removing a
  purchase always leaves a stamp, so the item's stamp cannot stand in for one:
  a later edit of the item by a device that has not seen the purchase keeps it
  (`cannot_express`). The item's own removal still takes it (`settle_orphans`).
- **Item removed beside a new purchase.** The later write wins, like for any
  field: a purchase stamped after the removal brings its item back (the item
  without the older purchases that were removed with it); a purchase stamped
  before the removal goes with the item. Both devices end equal.
- **Rule R6** (one link per operation) compares the stamp of the purchase, with
  the item's stamp as the fallback.

## Sync repair stage S

No protocol bump and no vault format change (`PROTOCOL_VERSION` stays 3,
`VAULT_FORMAT_VERSION` 3). Two pairs of protocol 3 builds, one before this stage
and one after it, still sync. What an older protocol 3 build sees:

| Change | On the wire | Older build |
|---|---|---|
| `PriceAlert.triggered_for` (an edited alert is armed again) | an unknown field is ignored | drops the field when it resaves the alert and treats it as triggered as before; the fix holds only while a new build owns the record |
| `SyncState.seen_by` (what a peer has seen, for "Overwritten") | not sent | none, it is local |
| "Overwritten" after a change of the record's kind | the summary is local | none |
| The editor's three-way save | ordinary stamped edits | takes them as the user's edits |
| Restore with live peers | ordinary stamped edits | takes them as the user's edits |
| `wf-off-<id>` run record (one per incomplete workflow, not one per device) | ordinary records | writes a `uuid` record; the extra ones are hidden by the `MAX_WORKFLOW_RUNS` trim |
| Rule R9 (a staged import already booked or discarded leaves the queue; `repair_imports.rs`) | ordinary deletions | takes them as the user's edits |
| `unstamped_conflicts` (external copies with no shared history) | the summary is local | none |

The vault gains only `PriceAlert.triggered_for` and `SyncState.seen_by`, both
`#[serde(default)]`; an old vault opens and keeps its data (`triggered_for`
absent means "triggered as before", an empty `seen_by` means no hiding).

Deletion loses to a live money reference: a payment that a shared purchase or
settlement names comes back (rule R3) even if another device deleted it. Pinned
in `repair_restore_payment_tests.rs`; the tests of this stage are
`seen_tests.rs`, `stale_base_tests.rs` (R9, the editor, restore) and
`unstamped_tests.rs` in `crates/core/src/service/peer_sync/`, and
`service/agenda/alert_tests.rs` (`triggered_for`).

## Sync repair stage P

No protocol bump and no vault format change (`PROTOCOL_VERSION` stays 3,
`VAULT_FORMAT_VERSION` 3). Builds before and after this stage still sync. What
an older protocol 3 build sees:

| Change | On the wire | Older build |
|---|---|---|
| `Settings.notifications` split into entries (`notifications.default_time`, `notifications.categories.<category>`) | new entries per category; the whole-object head is added for older receivers (`with_legacy_heads`, as for `marks`) | keeps the object as one value; the head stamp is the newest entry stamp and is a floor for entries without a stamp of their own; two older devices still lose one of two parallel switches |
| `notifications.system`, `notifications.hide_details_when_locked` under `skip` | never sent, never taken | keeps sending its own with the object; a new device ignores them and keeps its own |
| `dashboard_widgets`, `dashboard_period`, `pinned` under `skip` | never sent, never taken | keeps sending its own; a new device ignores them. It stops receiving the new device's layout |
| `Settings.inflation_manual` (flat rates by currency, split per currency) | additive entries `inflation_manual.<currency>` | does not know it; keeps its rates in its app config and computes with them; the group's rates do not reach it until it is updated |
| `Settings.inflation_adopted` under `skip` | never sent | none; a peer's flag does not cancel this device's adoption |
| `PriceStamp.price_base_currency` | field inside `price`, an unknown field is ignored | drops the label when it re-saves a transaction through its typed document; the number is then read as before (without a label) |
| `Workflow.max_amount_currency` | additive field of `workflows` | ignores it; the cap stays "in the base of the runner" |
| Text of an automatic reminder | the field `text` is the same | keeps rewriting the text in its own language; a new device does not rewrite in reply (a text equal to one of the two renderings is left alone, nothing is stamped) |
| Removing the password | local, the wire is untouched | none |

Scope of every setting. `sync::registry::SETTINGS_SCOPE` decides Group or
Device for every field of `Settings` (and for the parts of `notifications`);
`every_setting_has_a_scope` and `every_part_of_a_split_setting_has_a_scope` fail
for a new field without a line, `device_fields_are_skipped` requires every
Device name to be in the `skip` of the `settings` rule and no Group name there.

- Device: `display_decimals`, `fx_refresh_interval_sec`, `minimize_to_tray`
  (also in the app config), `dashboard_widgets`, `dashboard_period`, `pinned`,
  `inflation_adopted`, `notifications.system`,
  `notifications.hide_details_when_locked`.
- Group: everything else, among it `notifications.default_time`,
  `notifications.categories`, `inflation_mode`, `inflation_source`,
  `inflation_manual`.

The dashboard layout, its period and the pins stay in the encrypted vault, not
in `app_settings.json`: that file is plain JSON and a pinned filter holds text
the user typed. The cost: a full replacement of the vault by another copy
brings that copy's layout and pins.

Money labelled with a currency:

- **Price base (26).** `price_base_currency` is written wherever `price_base`
  is (the base of the device that entered it). `leg_price_in_base` keeps
  `price_base` when the label is missing or equals the reader's base; when it
  differs the price is read through `price_usd` and the reader's rate, and
  `price_base` is kept as is when `price_usd` or the rate is missing. A
  purchase of a property is signed with the currency of the label, the edit
  form takes its price currency from it. A record without a label (every
  record from before this stage, and one re-saved by an older build) reads as
  before. `budget_currency` is fixed when a manual income or expense is saved,
  because the figure is a number in the budget currency.
- **Cap (27).** `max_amount_currency` is filled from the saving device's base
  unless the form chose one. The run is checked in that currency: money already
  in the cap's currency needs no rate. A cap with no price or rate rejects the
  run, an old cap without a label included; an old cap is read in the base of
  the runner and gets the saving device's base as its label on the next save.
- **Inflation (100).** `flat_inflation()` is the shipped table with
  `inflation_manual` on top; every reader (goals, real returns, debts,
  property, the workflow node `data.inflation`, the health snapshot) uses it.
  On open the app layer calls `adopt_flat_inflation` once: rates of the app
  config that differ from the shipped ones and are not yet in the vault are
  copied into it. Two devices that adopted different values for one currency
  settle on the later write. `AppSettings.inflation` stays readable and is not
  written; `update_app_settings` ignores its `inflation`.

Tests: `peer_sync/notice_settings_tests.rs`, `peer_sync/inflation_tests.rs`,
`sync/registry.rs` (scope and path tests), the labelled-price tests in
`service/rates.rs` and `service/valuation.rs`, the cap tests in
`service/workflows/tests.rs`, `service/auto_reminders.rs`, and the convergence
test (`convergence_catalog.rs`: `NoticeSettings121`, `ReminderText33`,
`PriceBase26`, `FlatInflation100`; `KNOWN_OPEN` is empty).

## Sync repair stage C

No bump of the protocol or of the vault format. Only the connection and the
merge core changed.

| Change | On the wire | Older build |
|---|---|---|
| Silent peer in the exchange | `Bye "timeout"`, then a closed connection | treats any `Bye` as a quiet close and dials again |
| Limit of incoming connections (`MAX_INCOMING`) | the connection closes at once | sees a failed dial and backs off |
| Close of a live link | one more `Changed` before `Bye "closed"` | applies it normally |
| Order of a capped union (`discarded_imports`) | the same records | keeps other records at the cap and re-sends them; the new side trims again, extra traffic only |
| A record this build cannot read | left out and journaled once, the session goes on | fails the batch and ends every session with this device |
| Own entry in the peer list | still sent, it carries a revocation on | counts it as a device that never acks, so its tombstones stay |

Known limit (catalog 85). An older build drops a new optional field when it
rebuilds a record, so its digest never equals the newer build's. Each session
there gives a plan, a journal line and a toast until it is updated. A newer
build cannot fix this from its side.

## What an old peer still gets wrong

- It may drop an edited duplicate (the creation-stamp dedupe) and show secrets
  from labels the new side did not mask.
- It may run a scheduled job, a transaction trigger or a question a second time;
  the new side merges the result, but the old device may also post a duplicate
  that it never removes itself.
- It compacts tombstones by its own rule, so a deletion can come back from it.
- It re-sends thinned cache points every session.
- An old main sends a rank that grows with every client it has seen; the new
  client caps it at 5, so the takeover wait stays short.
- It applies records that arrive during a change question together with the
  answer.
- It does not understand "paused" or "forgotten" reasons, so it keeps knocking
  until it is updated or paired again.

The remedy is the same everywhere: update the app on every device.

Next: [Troubleshooting](troubleshooting.md).
