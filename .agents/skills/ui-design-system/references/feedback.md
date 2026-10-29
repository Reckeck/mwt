# Feedback: notifications, loading, errors, empty

An action that passed silently looks like a dead button. An action that failed
silently is worse: the user thinks everything was saved.

## A notification for every action

Everything that changes data (create, save, delete, archive, import, sync,
refresh prices, change a setting) ends with a notification. There is one
mechanism per application:

```ts
const archive = useMutation({
  mutationFn: (store: Store) => call('archive_store', { id: store.id, archived: !store.archived }),
  onSuccess: invalidate, // data only; no notification needed here
  meta: {
    error: 'stores.archiveFailed', // error title; the handler adds the reason
    success: (_data, store) => ({ text: t('stores.archivedToast'), description: (store as Store).name }),
    undo: (_data, store) => call('archive_store', { id: (store as Store).id, archived: (store as Store).archived }),
  },
})
```

- `MutationCache` (`app/providers/query`) shows success (`meta.success`: an
  i18n key, a text or a function -> text / `{ text, description }`; an empty
  string means "nothing to say", for example a file dialog was cancelled) and
  the error (`meta.error` + the reason by code via `anyErrorMessage`).
- The cache handler fires **before** the mutation's `onSuccess`, so
  `success` still sees the form state before it is reset.
- `meta.undo` — an "Undo" button for something reversible (archive, hide).
  A delete with no restore on the backend is a `ConfirmButton`, not `undo`.
- `meta.silent: '<why>'` — when the result is already on the screen (preview,
  layout autosave) or the mutation has its own, richer toast. `silent` also
  suppresses the error toast: the error is shown by the mutation's own
  `onError` via `notify.error`.
- A `success` string is an i18n key; a function returns the **finished** text
  (it calls `t()` itself), and its result is not translated again.
- The typical set is `{ success, error }`; `undo` is extra; `silent` replaces
  `success`.
- Do not call `toast` directly in a component. Outside mutations use `notify`
  (`shared/lib/notify`):

| Need | Call |
|---|---|
| success without a mutation | `notify.success('key' or text, { description })` |
| error from a `catch` | `notify.error(err, 'title.key')` |
| warning (no API key, nothing to import) | `notify.warning(...)` |
| neutral event from the backend | `notify.info(...)` |
| long operation | `notify.promise(promise, { loading, success, error })` |
| a setting that saves itself | `notify.saved()`: one "Saved" after a ~600 ms pause |

- `notify` drops a repeat of an identical message within 1.5 s: a double click
  does not give two toasts.
- Purely interface toggles (collapse a menu, a tab, expand a group, theme
  preview) give no notifications — the change is visible anyway.

## What is not a notification

A persistent state lives on the screen, not in a toast: an empty state, a sync
badge, a "rates are stale" warning, an error under a field. The toast will
disappear — information needed for longer must stay.

One event — one message: either a toast about the outcome, or text on the
screen about the state, not both.

## Loading, error, empty — `SectionState`

```vue
<SectionState
  :loading="positions.isPending.value"
  :error="positions.error.value"
  :empty="!rows.length"
  :empty-title="t('pos.empty')"
  :empty-description="t('pos.emptyHint')"
  skeleton="table"
  @retry="positions.refetch()"
>
  <DataTable :columns="columns" :data="rows" />
  <template #empty>
    <Button @click="createOpen = true">{{ t('tx.add') }}</Button>
  </template>
</SectionState>
```

- `loading` is only the first load (`isPending`); a background refresh leaves
  the data in place.
- The skeleton appears after ~200 ms: a fast answer does not flash a
  placeholder. The skeleton shape repeats the future content (`list`,
  `table`, `chart`, `figure`).
- An error is an `Alert` with the reason and "Retry"; do not hide it in a
  toast when there is no data on the screen.
- An empty state says what will be here and where to start, and gives a
  button for the first action. Empty because of a filter — "No data for the
  selected period" and a hint to change the filter (`ui.noData`,
  `ui.noDataHint`).
- A widget that shows `0` before loading lies: `StatCard :loading`.

## Waiting on a button

The button keeps its label and gets a `Spinner` + `disabled` while the request
runs. `FormDialog` already does this (`pending`).

## Dangerous actions

| Action | How |
|---|---|
| reversible (archive, hide) | immediately, a toast with "Undo" (`meta.undo`) |
| irreversible (delete) | `ConfirmButton`: click -> "Delete?" -> click; a toast with the result |
| the backend will refuse and the interface knows it (the wallet has transactions) | the button is disabled, the reason is next to it as text |
| the backend may refuse but the interface does not know in advance | the button is active; the refusal is an error toast with the reason and a hint ("archive the contact") |

`ConfirmButton`: the icon goes into the slot (the resting look), `label` is
its `aria-label` and hint, `confirm-label` is the text of the second press
("Delete?"). There is no pending flag: pass
`:disabled="remove.isPending.value"`, otherwise a quick double click runs the
mutation twice.

An "Are you sure?" modal is not used — except in one case: an irreversible
action from a context menu, where a two-step button has no place to live.
Then a short `Dialog size="sm"` with the action name on the button ("Delete
transaction"), and the menu item ends with "…" (the sample is deleting a
transaction in `pages/transactions/Transactions.vue`).
