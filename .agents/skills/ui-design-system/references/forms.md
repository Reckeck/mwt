# Формы (правила 13 и 14 AGENTS.md — здесь полный текст)

## Как устроена любая форма

Диалог, вкладка настроек или строка «добавить» — одинаково:

1. **Обязательное поле отмечено красной `*`**: `FormField required` или
   `Label required`; у `AssetInput` и `CurrencyPicker` — свои `required`/`error`.
2. **Кнопка сохранения активна.** Её блокирует только идущее сохранение
   (`isPending`) или отказ бэкенда, причина которого показана рядом.
   Пользователь не должен гадать, почему кнопка серая.
3. **Нажатие с пустым или неверным полем не сохраняет**, а показывает
   сообщение под каждым таким полем, красит рамку и ставит фокус на первое.
   Дальше сообщения следуют за вводом. Всё это даёт `useFormValidation`.
4. **Правила — из `shared/lib/form/validators.ts`**: `required`, `decimal`,
   `positiveAmount`, `nonNegativeAmount`, `integerInRange`, `minLength`,
   `firstError`. Тексты — `validation.*`, говорят, как исправить («Должно быть
   больше нуля»), а не «неверно».
5. **Что бэкенд отвергнет по известному правилу, форма проверяет сама**
   (пример: категория у пополнения и оплаты). «Invalid input» после нажатия —
   баг формы.
6. **Enter сохраняет** (в многострочном поле — Ctrl+Enter), Esc закрывает
   диалог.

## Диалог с формой — `FormDialog`

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
  { resetOn: open }, // заново открытый диалог начинается чистым
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

`FormDialog` даёт `<form>` (Enter), кнопки «Отмена» / «Сохранить», спиннер на
время сохранения и `@container` для раскладки полей. Свои кнопки слева в
подвале — слот `#footer-start`.

Без диалога: `<form @submit.prevent="form.submit(save.mutate, $event)">`, кнопка
вне формы — `@click="form.submit(save.mutate, $event)"`.

## Однострочная строка «добавить»

Обёрни каждый контрол в `<span class="contents" :data-invalid="form.invalidAttr('name')">`,
`form.firstMessage()` выведи одной строкой под рядом, после успешного
добавления вызови `form.reset()`, иначе очищенное поле сразу покажет ошибку.

## Редкие поля

Поля, которые заполняет меньшинство (комиссия, сеть и хэш, контакт,
кэшбэк), — под `AdvancedFields`. Если в скрытом поле уже есть значение или
ошибка, передай `default-open`, чтобы оно не пряталось.

## Числа и даты в полях

- Суммы — `AmountInput` (`inputmode="decimal"`, запятая и точка).
- Дата, дата со временем, месяц — `DateTimePicker` (`shared/ui/date-time-picker`,
  `mode="date|datetime|month"`), не `<Input type="date|datetime-local|month">`:
  модель — та же строка, что у нативного поля (`2026-10-14`,
  `2026-10-14T09:30`, `2026-10`), `min`/`max`, `required` (без «Очистить»),
  `invalid` для поля с ошибкой. Только время — `<Input type="time">`.
  В ISO для бэкенда — `dateInputToIso` / `toIsoTimestamp`, обратно —
  `localDate` / `toLocalInput` (`shared/lib/planning`, `shared/lib/datetime`).
  Необязательная дата — пустая строка → `null`.
- Коды, тикеры, хэши — `spellcheck="false"`, без автозаглавной.
- Плейсхолдер — пример значения, а не подпись: «0x…», «например, такси».

## Выбор счёта, актива, валюты (правило 14)

Везде, где пользователь выбирает счёт, актив или валюту (формы, фильтры,
настройки, поля нод сценариев), — один и тот же компонент:

| Что | Компонент | Заметки |
|---|---|---|
| кошелёк / субсчёт | `StoreNodeSelect` (`entities/store`) | путь «Кошелёк › Субсчёт»; `flat`, `placeholder`, `emptyLabel`, `invalid` |
| счёт, контакт или свободный текст в операции | `AccountRefPicker` (`features/transaction-create`) | |
| любой актив (поиск) | `AssetInput` (`entities/asset`) | `required`, `error` |
| один из известных (что лежит на счёте) | `AssetSelect` (`entities/asset`) | модель — `AssetRef`; `placeholder`, `invalid` |
| валюта | `CurrencyPicker` (`entities/currency`) | избранные сверху; `extra`, `kinds`, `allowEmpty`, `required`, `error` |

Как их класть в форму:

- `CurrencyPicker` и `AssetInput` рисуют свою подпись и ошибку (`label`,
  `required`, `error`) — **без** `FormField` вокруг.
- `StoreNodeSelect` и `AssetSelect` подписи не рисуют — оберни в
  `FormField` (`required`, `:error`) и передай `:invalid="!!form.error('x')"`.

Нужен вариант, которого компонент не умеет, — пропс в компонент, а не
список рядом.

## Проверка

`mise run check-forms` ловит: кнопку отправки, заблокированную чем-то кроме
`isPending`/`pending`/`busy`/`refused`/`loading`; `@submit` с ручной проверкой
(`valid && save()`); `<option v-for>` по счетам, активам, кодам валют;
`<Input>` с валютой в `v-model`.
