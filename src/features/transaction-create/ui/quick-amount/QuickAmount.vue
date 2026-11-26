<script setup lang="ts">
// check-ui-ignore intl-cache: one formatter per locale change, to read the decimal mark, as `AmountInput` does
import { computed, ref, watch } from 'vue'

import { evaluateAmountExpr, hasOperator } from '@/shared/lib/amount-expr'
import { normalizeAmount } from '@/shared/lib/amount-input'
import { activeLocale } from '@/shared/lib/locale'
import { useIsPhone } from '@/shared/lib/phone'
import { AmountInput } from '@/shared/ui/amount-input'
import { AmountKeypad } from '@/shared/ui/amount-keypad'

import { applyPadKey, padAmount, type PadKey } from '../../lib/keypad-text'

/**
 * The hero amount of the quick form. On a desktop it is the typing field with its inline
 * expression (`1200+350*2`); on a phone the field only shows, and the keypad under it types
 * (no system keyboard over half the sheet).
 */
const props = defineProps<{ id?: string; invalid?: boolean; ariaLabel?: string; placeholder?: string }>()
const model = defineModel<string>({ required: true })

const phone = useIsPhone()
const decimalMark = computed(
  () => new Intl.NumberFormat(activeLocale.value).formatToParts(1.5).find((part) => part.type === 'decimal')?.value ?? '.',
)

/** What the keypad has typed so far (`1200+35`); the model holds its value. */
const typed = ref(model.value)
watch(model, (value) => {
  // Cleared or filled from outside (a series' next operation, a prefill).
  if (padAmount(typed.value) !== value) typed.value = value
})

function press(key: PadKey) {
  typed.value = applyPadKey(typed.value, key)
  const amount = padAmount(typed.value)
  if (amount !== null) model.value = amount
}

const expression = computed(() => (hasOperator(typed.value) ? typed.value.replace(/\*/g, '×').replace(/\//g, '÷') : ''))
const result = computed(() => {
  if (!hasOperator(typed.value)) return ''
  const outcome = evaluateAmountExpr(typed.value)
  return outcome.ok ? `= ${normalizeAmount(outcome.value, { decimalMark: decimalMark.value }).display}` : ''
})
</script>

<template>
  <div class="grid gap-3">
    <AmountInput
      :id="props.id"
      v-model="model"
      size="hero"
      :calculator="false"
      :placeholder="props.placeholder ?? '0'"
      :aria-label="props.ariaLabel"
      :aria-invalid="props.invalid || undefined"
      :class="props.invalid && 'border-destructive'"
      :readonly="phone || undefined"
      :inputmode="phone ? 'none' : 'decimal'"
      data-quick-amount
    >
      <template v-if="$slots.suffix" #suffix><slot name="suffix" /></template>
    </AmountInput>
    <p v-if="phone && expression" class="min-h-5 text-right text-sm tabular-nums text-muted-foreground" data-quick-expr>
      {{ expression }} {{ result }}
    </p>
    <AmountKeypad
      :decimal-mark="decimalMark"
      @digit="press({ kind: 'digit', value: $event })"
      @decimal="press({ kind: 'decimal' })"
      @backspace="press({ kind: 'backspace' })"
      @operator="press({ kind: 'operator', value: $event })"
    />
  </div>
</template>
