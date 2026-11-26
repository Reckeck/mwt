<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { evaluateAmountExpr, hasOperator } from '@/shared/lib/amount-expr'
import { normalizeAmount } from '@/shared/lib/amount-input'
import { decimalMark } from '@/shared/lib/decimal-mark'
import { useIsPhone } from '@/shared/lib/phone'
import { AmountInput } from '@/shared/ui/amount-input'

import { applyPadKey, padAmount, type PadKey } from '../../lib/keypad-text'

/**
 * An amount of the quick form: the hero (a phone), the bold box of board 533527 (a desktop) or the
 * right half of an exchange box (`inline`). On a desktop it is the typing field with its inline
 * expression (`1200+350*2`); on a phone the field only shows, and the form's keypad types into it
 * through the exposed `press` (no system keyboard over half the sheet).
 */
const props = withDefaults(
  defineProps<{ id?: string; invalid?: boolean; ariaLabel?: string; placeholder?: string; size?: 'hero' | 'box' | 'inline' }>(),
  { id: undefined, ariaLabel: undefined, placeholder: '0', size: 'hero' },
)
const model = defineModel<string>({ required: true })
const phone = useIsPhone()

/** What the keypad has typed so far (`1200+35`); the model holds its value. */
const typed = ref(model.value)
watch(model, (value) => {
  // Cleared or filled from outside (a series' next operation, a prefill, the desktop keyboard).
  if (padAmount(typed.value) !== value) typed.value = value
})

function press(key: PadKey) {
  typed.value = applyPadKey(typed.value, key)
  const amount = padAmount(typed.value)
  if (amount !== null) model.value = amount
}
defineExpose({ press })

const expression = computed(() => (hasOperator(typed.value) ? typed.value.replace(/\*/g, '×').replace(/\//g, '÷') : ''))
const result = computed(() => {
  if (!hasOperator(typed.value)) return ''
  const outcome = evaluateAmountExpr(typed.value)
  return outcome.ok ? `= ${normalizeAmount(outcome.value, { decimalMark: decimalMark() }).display}` : ''
})
</script>

<template>
  <div class="grid min-w-0 gap-1">
    <AmountInput
      :id="props.id"
      v-model="model"
      :size="props.size"
      :calculator="false"
      :placeholder="props.placeholder"
      :aria-label="props.ariaLabel"
      :aria-invalid="props.invalid || undefined"
      :class="props.invalid && props.size !== 'inline' && 'border-destructive'"
      :readonly="phone || undefined"
      :inputmode="phone ? 'none' : 'decimal'"
      data-quick-amount
    >
      <template v-if="$slots.suffix" #suffix><slot name="suffix" /></template>
    </AmountInput>
    <p v-if="phone && expression" class="text-right text-xs tabular-nums text-muted-foreground" data-quick-expr>{{ expression }} {{ result }}</p>
  </div>
</template>
