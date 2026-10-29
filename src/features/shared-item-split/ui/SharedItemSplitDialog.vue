<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { type SplitMode, checkUnits, splitMode, splitOf, sumDecimals, unitsTotal, useSetSharedItemSplit } from '@/entities/shared-event'
import { decimalMul, parseDecimal } from '@/shared/lib/decimal'
import { useFormValidation } from '@/shared/lib/form'
import { formatMoney, formatQuantity } from '@/shared/lib/format'
import { Checkbox } from '@/shared/ui/checkbox'
import { Label } from '@/shared/ui/label'
import { FormDialog } from '@/shared/ui/form-dialog'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'

import type { SharedItemSplitDialogProps } from '../shared-item-split.types'
import SplitUnitsRow from './SplitUnitsRow.vue'

/**
 * «Как делить · Пиво»: everybody equally, only the people who took part, or by units («из 8 шт Петя
 * взял 3, Вася 5»). The shares of the event are recalculated at once; «Отменить» restores the old split.
 */
const props = defineProps<SharedItemSplitDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const mode = ref<SplitMode>('all')
const among = ref<string[]>([])
const units = ref<Record<string, string>>({})

const view = computed(() => props.event.items.find((entry) => entry.item_id === props.item?.id))
const bought = computed(() => view.value?.qty ?? '0')
const hasPurchase = computed(() => Boolean(view.value?.bought))
const currency = computed(() => props.event.event.currency)
const decimals = computed(() => sumDecimals(props.event))
const people = computed(() => props.event.people.map((person) => ({ id: person.id, name: person.name || t('sharedEvents.me') })))
const unit = computed(() => props.item?.unit ?? '')

watch(open, (now) => {
  if (!now || !props.item) return
  const split = props.item.split
  mode.value = splitMode(split)
  among.value = 'equal' in split ? [...split.equal.among] : []
  units.value = 'by_units' in split ? { ...split.by_units.units } : {}
})

/** What one unit of the item came to. */
const unitPrice = computed(() => {
  const count = Number(parseDecimal(bought.value) ?? 0)
  const cost = Number(view.value?.cost ?? 0)
  return count > 0 ? String(cost / count) : '0'
})
// money: plain — a text line under the option
const unitPriceText = computed(() =>
  t('sharedEvents.split.unitPrice', { price: formatMoney(unitPrice.value, { currency: currency.value, decimals: decimals.value }), unit: unit.value }),
)
// money: plain — a text line under the option
const equalText = computed(() => {
  const count = people.value.length
  const share = count ? String(Number(view.value?.cost ?? 0) / count) : '0'
  return t('sharedEvents.split.equalEach', { price: formatMoney(share, { currency: currency.value, decimals: decimals.value }), count })
})
// money: plain — the description is text
const description = computed(() =>
  view.value && hasPurchase.value
    ? t('sharedEvents.split.summary', {
        qty: formatQuantity(bought.value),
        unit: unit.value,
        cost: formatMoney(view.value.cost, { currency: currency.value, decimals: decimals.value }),
      })
    : undefined,
)
const check = computed(() => checkUnits(units.value, bought.value))
const handedOut = computed(() => unitsTotal(units.value))

const form = useFormValidation(
  () => ({
    some: mode.value === 'some' && !among.value.length ? t('sharedEvents.split.pickSome') : null,
    units:
      mode.value !== 'units' || check.value.kind === 'ok'
        ? null
        : check.value.kind === 'empty'
          ? t('sharedEvents.split.emptyUnits')
          : check.value.kind === 'short'
            ? t('sharedEvents.split.shortUnits', { left: formatQuantity(check.value.left), unit: unit.value })
            : t('sharedEvents.split.overUnits', { extra: formatQuantity(check.value.extra), unit: unit.value }),
  }),
  { resetOn: open },
)

function togglePerson(id: string, on: boolean) {
  among.value = on ? [...new Set([...among.value, id])] : among.value.filter((entry) => entry !== id)
}
const amountOf = (id: string) => decimalMul(parseDecimal(units.value[id] ?? '') ?? '0', unitPrice.value) ?? '0'

const mutation = useSetSharedItemSplit()
function save() {
  if (!props.item) return
  mutation.mutate(
    {
      input: { event_id: props.event.event.id, item_id: props.item.id, split: splitOf(mode.value, among.value, units.value) },
      previous: props.item.split,
    },
    { onSuccess: () => (open.value = false) },
  )
}

const HEAD = 'flex cursor-pointer items-start gap-3 font-normal'
const CARD = 'grid gap-3 rounded-xl border border-border p-3 has-[[data-state=checked]]:border-primary'
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-split"
    form-id="shared-split-form"
    size="sm"
    :title="t('sharedEvents.split.title', { title: props.item?.title ?? '' })"
    :description="description"
    :submit-label="t('actions.save')"
    :pending="mutation.isPending.value"
    @submit="form.submit(save, $event)"
  >
    <RadioGroup v-model="mode" class="grid gap-3">
      <div :class="CARD">
        <Label for="split-all" :class="HEAD">
          <RadioGroupItem id="split-all" value="all" class="mt-0.5" />
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t('sharedEvents.split.all') }}</span>
            <span v-if="hasPurchase" class="text-xs font-normal text-muted-foreground">{{ equalText }}</span>
          </span>
        </Label>
      </div>

      <div :class="CARD">
        <Label for="split-some" :class="HEAD">
          <RadioGroupItem id="split-some" value="some" class="mt-0.5" />
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t('sharedEvents.split.some') }}</span>
            <span class="text-xs font-normal text-muted-foreground">{{ t('sharedEvents.split.someHint') }}</span>
          </span>
        </Label>
        <div v-if="mode === 'some'" class="grid gap-1 pl-7" :data-invalid="form.error('some') ? '' : undefined">
          <Label v-for="person in people" :key="person.id" :for="`split-person-${person.id}`" class="flex min-h-9 cursor-pointer items-center gap-3 font-normal">
            <Checkbox
              :id="`split-person-${person.id}`"
              :model-value="among.includes(person.id)"
              @update:model-value="(on) => togglePerson(person.id, on === true)"
            />
            {{ person.name }}
          </Label>
          <p v-if="form.error('some')" class="text-xs text-destructive" role="alert">{{ form.error('some') }}</p>
        </div>
      </div>

      <div v-if="hasPurchase" :class="CARD">
        <Label for="split-units" :class="HEAD">
          <RadioGroupItem id="split-units" value="units" class="mt-0.5" />
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t('sharedEvents.split.units') }}</span>
            <span class="text-xs font-normal text-muted-foreground">{{ unitPriceText }}</span>
          </span>
        </Label>
        <div v-if="mode === 'units'" class="grid gap-2 pl-0 @sm:pl-7" :data-invalid="form.error('units') ? '' : undefined">
          <SplitUnitsRow
            v-for="person in people"
            :key="person.id"
            :model-value="units[person.id] ?? ''"
            :name="person.name"
            :unit="unit"
            :amount="amountOf(person.id)"
            :currency="currency"
            :decimals="decimals"
            @update:model-value="(value) => (units = { ...units, [person.id]: value })"
          />
          <p v-if="form.error('units')" class="text-xs text-destructive" role="alert">{{ form.error('units') }}</p>
          <p v-else-if="check.kind === 'ok'" class="flex items-center gap-1 text-xs font-semibold text-success">
            <Check class="size-4" aria-hidden="true" />
            {{ t('sharedEvents.split.handedOut', { got: formatQuantity(handedOut), total: formatQuantity(bought), unit }) }}
            <Money :amount="view?.cost ?? '0'" :currency="currency" :decimals="decimals" :convert="false" />
          </p>
        </div>
      </div>
    </RadioGroup>
  </FormDialog>
</template>
