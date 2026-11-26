<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useCategories } from '@/shared/api/queries'
import { assetKey } from '@/shared/lib/asset-ref'
import { categoryOptions } from '@/shared/lib/categories'
import { decimalMark } from '@/shared/lib/decimal-mark'
import { currencySymbol } from '@/shared/lib/format'
import type { FieldError } from '@/shared/lib/form'
import { useIsPhone } from '@/shared/lib/phone'
import { AmountKeypad } from '@/shared/ui/amount-keypad'
import { FormField } from '@/shared/ui/form-field'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import type { PadKey } from '../../lib/keypad-text'
import { isTwoWalletKind, quickAssetStoreId, type QuickField, type QuickFormState } from '../../lib/quick-form'
import { byUsage } from '../../lib/frequent-category'
import { useFrequentCategory } from '../../model/use-frequent-category'
import { useQuickAsset } from '../../model/use-quick-asset'
import { AccountRefPicker } from '../account-ref-picker'
import { ExchangeSide } from '../exchange-side'
import { QuickAmount } from '../quick-amount'
import { QuickDateChips } from '../quick-date'
import { QuickNote } from '../quick-note'
import { TransferSideField } from '../transfer-side'

/**
 * The body of the quick add (boards 533527, 533927, 531451): three tabs, the amount (a box on a
 * desktop, the hero on a phone), the wallet plate, the category chips, a row of chips for the day,
 * the note and «Ещё», and on a phone the keypad at the bottom. No field labels but the two sides of a
 * transfer. A transfer between wallets of two different assets is an exchange (boards 462813,
 * 460801): two boxes «отдал» and «получил», each a wallet and an amount, and the deal rate under them;
 * the keypad types the box last touched. It validates
 * nothing itself: it shows what the owner passes in `errors` (after a save was tried, rule 13). The
 * pieces of sibling features come in through slots: `#category`, `#contact` (a side of a transfer),
 * `#exchange` (what was given and the two asset symbols) and `#more` (the last chip of the row).
 */
defineSlots<{
  category(props: { options: { value: string; label: string }[]; invalid: boolean }): unknown
  contact(props: { side: 'from' | 'to'; id: string; invalid: boolean }): unknown
  exchange(props: { gave: string; got: string; from: string; to: string }): unknown
  more(): unknown
}>()
const props = defineProps<{ errors: Partial<Record<QuickField, FieldError | undefined>> }>()
const form = defineModel<QuickFormState>('form', { required: true })
const { t } = useI18n()
const phone = useIsPhone()

const TABS = ['payment', 'topup', 'transfer'] as const
type Tab = (typeof TABS)[number]
const tab = computed<Tab>(() => (form.value.kind === 'convert' ? 'transfer' : form.value.kind))

const categories = useCategories()
const GROUPS = { payment: ['daily', 'fixed', 'savings'], topup: ['income', 'savings'] } as const
const usage = useFrequentCategory(() => (form.value.kind === 'topup' ? 'topup' : 'payment')).usage
/** The categories of the kind, the most used lately first (the chips show the first few). */
const options = computed(() => {
  const kind = form.value.kind
  return kind === 'payment' || kind === 'topup' ? byUsage(categoryOptions(categories.data.value, t, GROUPS[kind]), usage.value) : []
})
const exchange = computed(() => form.value.kind === 'convert')
const gaveAsset = useQuickAsset(() => form.value.storeId)
const gotAsset = useQuickAsset(() => form.value.toStoreId)
const amountAsset = useQuickAsset(() => quickAssetStoreId(form.value))
const currency = computed(() => (amountAsset.value ? currencySymbol(amountAsset.value.symbol) : ''))
const gotCurrency = computed(() => (gotAsset.value ? currencySymbol(gotAsset.value.symbol) : ''))
const gaveCaption = (balance: string) => t('quickAdd.side.gave', { balance })
const gotCaption = computed(() => t(form.value.amount && form.value.toAmount ? 'quickAdd.side.gotRated' : 'quickAdd.side.got'))

/** The amount the keypad types: the one amount, or the exchange box last touched. */
type PadTarget = { press: (key: PadKey) => void } | null
const amountField = ref<PadTarget>(null)
const gotField = ref<PadTarget>(null)
const typing = ref<'amount' | 'got'>('amount')
watch(exchange, (isExchange) => {
  if (!isExchange) typing.value = 'amount'
})
function press(key: PadKey) {
  ;(typing.value === 'got' ? gotField.value : amountField.value)?.press(key)
}
// A category of the other kind does not carry over.
watch(options, (list) => {
  if (form.value.category && !list.some((option) => option.value === form.value.category)) form.value.category = ''
})
// Two wallets of one asset are a transfer, of two assets an exchange; a contact side is always a transfer.
watch(
  [() => form.value.kind, gaveAsset, gotAsset, () => form.value.fromContactId, () => form.value.toContactId],
  ([kind, gave, got, fromContact, toContact]) => {
    if (!isTwoWalletKind(kind)) return
    if (fromContact || toContact) form.value.kind = 'transfer'
    else if (gave && got) form.value.kind = assetKey(gave) === assetKey(got) ? 'transfer' : 'convert'
  },
)

/** Clicking the lit segment would clear it; a kind always stays picked. */
function pickTab(value: unknown) {
  if (typeof value === 'string' && TABS.includes(value as Tab) && value !== tab.value) form.value.kind = value as Tab
}
</script>

<template>
  <div class="grid gap-4">
    <ToggleGroup type="single" variant="segmented" :spacing="1" class="w-full" data-quick-kind :model-value="tab" @update:model-value="pickTab">
      <ToggleGroupItem v-for="kind in TABS" :key="kind" :value="kind" class="flex-1" :data-kind="kind">
        {{ t(`quickAdd.kind.${kind}`) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <template v-if="exchange">
      <FormField :error="props.errors.amount ?? props.errors.storeId ?? undefined">
        <ExchangeSide
          ref="amountField"
          v-model:wallet="form.storeId"
          v-model:amount="form.amount"
          store-id="quick-store"
          amount-id="quick-amount"
          :caption="gaveCaption"
          :currency="currency"
          :label="t('form.sum')"
          :active="typing === 'amount'"
          :store-invalid="Boolean(props.errors.storeId)"
          :amount-invalid="Boolean(props.errors.amount)"
          @activate="typing = 'amount'"
        />
      </FormField>
      <FormField :error="props.errors.toAmount ?? props.errors.toStoreId ?? undefined">
        <ExchangeSide
          ref="gotField"
          v-model:wallet="form.toStoreId"
          v-model:amount="form.toAmount"
          store-id="quick-to-store"
          amount-id="quick-got"
          :caption="gotCaption"
          :currency="gotCurrency"
          :label="t('quickAdd.got')"
          :active="typing === 'got'"
          :store-invalid="Boolean(props.errors.toStoreId)"
          :amount-invalid="Boolean(props.errors.toAmount)"
          @activate="typing = 'got'"
        />
      </FormField>
      <slot name="exchange" :gave="form.amount" :got="form.toAmount" :from="gaveAsset?.symbol.toLowerCase() ?? ''" :to="gotAsset?.symbol.toLowerCase() ?? ''" />
    </template>
    <template v-else>
      <FormField :error="props.errors.amount ?? undefined">
        <QuickAmount ref="amountField" id="quick-amount" v-model="form.amount" :size="phone ? 'hero' : 'box'" :invalid="Boolean(props.errors.amount)" :aria-label="t('form.sum')">
          <template v-if="currency" #suffix>
            <span class="text-2xl text-muted-foreground">{{ currency }}</span>
          </template>
        </QuickAmount>
      </FormField>
      <FormField v-if="!isTwoWalletKind(form.kind)" :error="props.errors.storeId ?? undefined">
        <AccountRefPicker id="quick-store" v-model="form.storeId" variant="plate" :placeholder="t('quickAdd.store')" :invalid="Boolean(props.errors.storeId)" />
      </FormField>
      <template v-else>
        <TransferSideField
          id="quick-store"
          v-model:wallet="form.storeId"
          v-model:contact="form.fromContactId"
          :label="t('tx.quick.fromWallet')"
          :can-contact="!form.toContactId"
          :error="props.errors.storeId ?? undefined"
        >
          <template #contact="{ id, invalid }"><slot name="contact" side="from" :id="id" :invalid="invalid" /></template>
        </TransferSideField>
        <TransferSideField
          id="quick-to-store"
          v-model:wallet="form.toStoreId"
          v-model:contact="form.toContactId"
          :label="t('tx.quick.toWallet')"
          :can-contact="!form.fromContactId"
          :error="props.errors.toStoreId ?? undefined"
        >
          <template #contact="{ id, invalid }"><slot name="contact" side="to" :id="id" :invalid="invalid" /></template>
        </TransferSideField>
      </template>
      <FormField v-if="!isTwoWalletKind(form.kind)" :error="props.errors.category ?? undefined">
        <slot name="category" :options="options" :invalid="Boolean(props.errors.category)" />
      </FormField>
    </template>

    <FormField :error="props.errors.timestamp ?? undefined">
      <div class="flex flex-wrap gap-2" data-quick-chips>
        <QuickDateChips id="quick-date" v-model="form.timestamp" :invalid="Boolean(props.errors.timestamp)" />
        <QuickNote id="quick-note" v-model="form.note" />
        <slot name="more" />
      </div>
    </FormField>
    <AmountKeypad
      :decimal-mark="decimalMark()"
      @digit="press({ kind: 'digit', value: $event })"
      @decimal="press({ kind: 'decimal' })"
      @backspace="press({ kind: 'backspace' })"
      @operator="press({ kind: 'operator', value: $event })"
    />
  </div>
</template>
