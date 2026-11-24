<script setup lang="ts">
import { Delete } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'

import { KEYPAD_ROWS, type KeypadKey, type KeypadOperator } from './keypad-keys'

const OPERATOR_LABEL_KEY: Record<KeypadOperator, string> = {
  '+': 'amount.calc.op.plus',
  '-': 'amount.calc.op.minus',
  '*': 'amount.calc.op.multiply',
  '/': 'amount.calc.op.divide',
}

/**
 * The on-screen number pad of a phone (quick add, amount forms): digits, the
 * decimal mark, backspace and a column of `+ - x /`. It only reports what was
 * pressed; the owner applies the events to its own text. Desktop has a
 * keyboard, so nothing is rendered there.
 */
const props = defineProps<{ decimalMark: string }>()
const emit = defineEmits<{
  digit: [digit: string]
  decimal: []
  backspace: []
  operator: [operator: KeypadOperator]
}>()
const { t } = useI18n()
const phone = useIsPhone()

function press(key: KeypadKey) {
  if (key.kind === 'digit') emit('digit', key.value)
  else if (key.kind === 'operator') emit('operator', key.value)
  else if (key.kind === 'decimal') emit('decimal')
  else emit('backspace')
}

function label(key: KeypadKey): string {
  if (key.kind === 'decimal') return props.decimalMark
  if (key.kind === 'backspace') return t('amount.calc.back')
  return key.label
}

function ariaLabel(key: KeypadKey): string {
  if (key.kind === 'operator') return t(OPERATOR_LABEL_KEY[key.value])
  return label(key)
}
</script>

<template>
  <div v-if="phone" class="grid grid-cols-4 gap-2" data-amount-pad>
    <template v-for="row in KEYPAD_ROWS" :key="row.map((key) => key.id).join('')">
      <Button
        v-for="key in row"
        :key="key.id"
        type="button"
        :variant="key.kind === 'operator' ? 'secondary' : 'outline'"
        class="text-keypad h-14 min-w-10"
        :aria-label="ariaLabel(key)"
        :data-pad-key="key.id"
        @click="press(key)"
      >
        <Delete v-if="key.kind === 'backspace'" class="size-6" />
        <template v-else>{{ label(key) }}</template>
      </Button>
    </template>
  </div>
</template>
