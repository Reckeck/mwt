# Forms (rules 13 and 14 of AGENTS.md — the full text is here)

## How any form is built

A dialog, a settings tab or an "add" row — all the same:

1. **A required field is marked with a red `*`**: `FormField required` or
   `Label required`; `AssetInput` and `CurrencyPicker` have their own
   `required`/`error`.
2. **The save button is active.** It is blocked only by a save in progress
   (`isPending`) or a backend refusal whose reason is shown next to it. The
   user must not guess why the button is gray.
3. **A press with an empty or wrong field does not save**, but shows a message
   under each such field, colors the border and puts focus on the first one.
   After that the messages follow the input. All of this is provided by
   `useFormValidation`.
4. **The rules come from `shared/lib/form/validators.ts`**: `required`,
   `decimal`, `positiveAmount`, `nonNegativeAmount`, `integerInRange`,
   `minLength`, `firstError`. The texts are `validation.*`; they say how to
   fix it ("Must be greater than zero"), not "invalid".
5. **What the backend will reject by a known rule, the form checks itself**
   (example: a category on a top-up and a payment). "Invalid input" after a
   press is a form bug.
6. **Enter saves** (in a multiline field — Ctrl+Enter), Esc closes the dialog.

## A dialog with a form — `FormDialog`

```vue
<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { CurrencyPicker } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const limit = ref('')
const currency = ref('')

// `resetOn` clears only the "save was tried" flag; the values are reset here.
watch(open, (isOpen) => {
  if (!isOpen) return
  limit.value = ''
  currency.value = ''
})

const form = useFormValidation(
  () => ({
    name: required(name.value),
    limit: positiveAmount(limit.value),
    currency: required(currency.value, 'pick'),
  }),
  { resetOn: open }, // a reopened dialog starts clean
)
const save = useMutation({
  mutationFn: () => call<Budget>('create_budget', { input: { limit: limit.value, currency: currency.value } }),
  onSuccess: () => {
    open.value = false
  },
  meta: { success: 'budget.created', error: 'budget.saveFailed' },
})
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('budget.new')"
    form-id="budget-form"
    :pending="save.isPending.value"
    @submit="form.submit(save.mutate, $event)"
  >
    <FormField for="budget-limit" :label="t('budget.limit')" required :error="form.error('limit')">
      <AmountInput id="budget-limit" v-model="limit" />
    </FormField>
    <!-- CurrencyPicker draws its own label: no FormField around it. -->
    <CurrencyPicker v-model="currency" :label="t('budget.currency')" required :error="form.error('currency')" />
  </FormDialog>
</template>
```

`FormDialog` provides the `<form>` (Enter), the "Cancel" / "Save" buttons, a
spinner while saving and a `@container` for the field layout. Your own buttons
on the left of the footer go in the `#footer-start` slot.

Without a dialog: `<form @submit.prevent="form.submit(save.mutate, $event)">`,
a button outside the form — `@click="form.submit(save.mutate, $event)"`.

## A single-line "add" row

Wrap each control in `<span class="contents" :data-invalid="form.invalidAttr('name')">`,
print `form.firstMessage()` as one line under the row, and after a successful
add call `form.reset()`, otherwise the cleared field will immediately show an
error.

## Rare fields

Fields that a minority fill in (fee, network and hash, contact, cashback) go
under `AdvancedFields`. If a hidden field already has a value or an error,
pass `default-open` so it does not hide.

## Numbers and dates in fields

- Amounts — `AmountInput` (`inputmode="decimal"`, comma and dot).
- Date, date with time, month — `DateTimePicker`
  (`shared/ui/date-time-picker`, `mode="date|datetime|month"`), not
  `<Input type="date|datetime-local|month">`: the model is the same string as
  the native field's (`2026-10-14`, `2026-10-14T09:30`, `2026-10`),
  `min`/`max`, `required` (without "Clear"), `invalid` for a field with an
  error. Time only — `<Input type="time">`. To ISO for the backend —
  `dateInputToIso` / `toIsoTimestamp`, back — `localDate` / `toLocalInput`
  (`shared/lib/planning`, `shared/lib/datetime`). An optional date is an empty
  string -> `null`.
- Codes, tickers, hashes — `spellcheck="false"`, no auto-capitalization.
- A placeholder is an example value, not a label: "0x…", "for example, taxi".

## Choosing an account, asset, currency (rule 14)

Everywhere the user chooses an account, asset or currency (forms, filters,
settings, workflow node fields) it is the same component:

| What | Component | Notes |
|---|---|---|
| wallet / sub-account | `StoreNodeSelect` (`entities/store`) | path "Wallet › Sub-account"; `flat`, `placeholder`, `emptyLabel`, `invalid` |
| account, contact or free text in a transaction | `AccountRefPicker` (`features/transaction-create`) | |
| any asset (search) | `AssetInput` (`entities/asset`) | `required`, `error` |
| one of the known ones (what lies on the account) | `AssetSelect` (`entities/asset`) | the model is `AssetRef`; `placeholder`, `invalid` |
| currency | `CurrencyPicker` (`entities/currency`) | favorites on top; `extra`, `kinds`, `allowEmpty`, `required`, `error` |

How to put them into a form:

- `CurrencyPicker` and `AssetInput` draw their own label and error (`label`,
  `required`, `error`) — **without** a `FormField` around them.
- `StoreNodeSelect` and `AssetSelect` do not draw labels — wrap them in a
  `FormField` (`required`, `:error`) and pass
  `:invalid="!!form.error('x')"`.

If you need a variant the component cannot do, add a prop to the component,
not a list next to it.

## Check

`mise run check-forms` catches: a submit button blocked by anything except
`isPending`/`pending`/`busy`/`refused`/`loading`; `@submit` with a manual
check (`valid && save()`); `<option v-for>` over accounts, assets, currency
codes; an `<Input>` with a currency in `v-model`.
