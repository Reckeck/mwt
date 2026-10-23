<script setup lang="ts">
import { computed, nextTick, ref, useAttrs, watch } from 'vue'

import {
  GROUP_SEPARATOR,
  caretAfterSignificant,
  normalizeAmount,
  significantBefore,
} from '@/shared/lib/amount-input'
import { activeLocale } from '@/shared/lib/locale'
import { cn } from '@/shared/lib/utils'

import type { AmountInputProps } from './amount-input.types'

/**
 * A number field that groups digits as you type (`12 345,67`).
 *
 * The model is always a plain decimal string (`12345.67`) that `Amount` parses
 * as is; the grouped text exists only inside the field. Both `,` and `.` are
 * accepted as the decimal mark, and pasted amounts like `1,234.50 ₽` are read
 * correctly. The `suffix` slot sits inside the field's border (a currency).
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<AmountInputProps>(), {
  decimals: undefined,
  allowNegative: false,
  placeholder: '',
  disabled: false,
  id: undefined,
  size: 'default',
})
const model = defineModel<string | number>({ default: '' })

const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})

const decimalMark = computed(() => {
  const part = new Intl.NumberFormat(activeLocale.value).formatToParts(1.5).find((entry) => entry.type === 'decimal')
  return part?.value ?? '.'
})
const options = computed(() => ({
  decimals: props.decimals,
  allowNegative: props.allowNegative,
  decimalMark: decimalMark.value,
}))

const display = ref('')

/** Outside changes (form reset, prefill) are reformatted; our own echo is not. */
watch(
  [() => model.value, options],
  ([value]) => {
    const text = value === null || value === undefined ? '' : String(value)
    const next = normalizeAmount(text, options.value)
    if (next.canonical !== normalizeAmount(display.value, options.value).canonical || !display.value) {
      display.value = next.display
    }
  },
  { immediate: true },
)

function onInput(event: Event) {
  const target = event.target as HTMLInputElement
  const caret = target.selectionStart ?? target.value.length
  const keep = significantBefore(target.value, caret)
  const next = normalizeAmount(target.value, options.value)
  display.value = next.display
  target.value = next.display
  model.value = next.canonical
  const position = caretAfterSignificant(next.display, keep)
  void nextTick(() => target.setSelectionRange(position, position))
}

/** Backspace right after a group space deletes the digit before it, not the space. */
function onKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLInputElement
  const start = target.selectionStart ?? 0
  if (start !== target.selectionEnd) return
  if (event.key === 'Backspace' && target.value[start - 1] === GROUP_SEPARATOR) {
    target.setSelectionRange(start - 1, start - 1)
  } else if (event.key === 'Delete' && target.value[start] === GROUP_SEPARATOR) {
    target.setSelectionRange(start + 1, start + 1)
  }
}
</script>

<template>
  <div
    :class="
      cn(
        'flex h-9 w-full min-w-0 items-center rounded-md border border-input bg-transparent shadow-sm transition-colors focus-within:ring-1 focus-within:ring-ring has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-50',
        props.size === 'lg' && 'h-12',
        cls,
      )
    "
  >
    <input
      :id="props.id"
      :value="display"
      type="text"
      inputmode="decimal"
      autocomplete="off"
      spellcheck="false"
      :placeholder="props.placeholder"
      :disabled="props.disabled"
      :class="
        cn(
          'h-full w-0 min-w-0 flex-1 bg-transparent px-3 py-1 text-sm tabular-nums outline-none placeholder:text-muted-foreground',
          props.size === 'lg' && 'text-xl font-semibold',
        )
      "
      v-bind="rest"
      @input="onInput"
      @keydown="onKeydown"
    />
    <div v-if="$slots.suffix" class="flex shrink-0 items-center pr-1">
      <slot name="suffix" />
    </div>
  </div>
</template>
