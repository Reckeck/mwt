<script setup lang="ts">
import { onScopeDispose, ref } from 'vue'
import { useEventListener } from '@vueuse/core'
import { TooltipRoot } from 'reka-ui'

import { TooltipContent } from '@/shared/ui/tooltip'

import { MoneyConversions } from '../money-conversions'

/**
 * The one conversions tooltip for every `<Money>` on screen: a page has
 * hundreds of sums, and a tooltip each was slow to appear and swapped the
 * sum's element on first hover. This host watches the pointer, waits like any
 * tooltip, and shows the conversions next to the sum it rests on.
 */
const OPEN_DELAY_MS = 300
const SUM = '[data-money-convertible]'

const open = ref(false)
const target = ref<HTMLElement | null>(null)
const amount = ref(0)
const currency = ref('')
let showTimer: ReturnType<typeof setTimeout> | undefined
let hideTimer: ReturnType<typeof setTimeout> | undefined

function sumOf(node: EventTarget | null): HTMLElement | null {
  return node instanceof Element ? node.closest<HTMLElement>(SUM) : null
}

function show(sum: HTMLElement) {
  target.value = sum
  amount.value = Number(sum.dataset.moneyAmount)
  currency.value = sum.dataset.moneyCurrency ?? ''
  open.value = true
}

useEventListener(document, 'pointerover', (event: PointerEvent) => {
  // A finger has no hover: the tooltip would only flash on a tap.
  if (event.pointerType === 'touch') return
  const sum = sumOf(event.target)
  if (!sum) return
  clearTimeout(hideTimer)
  if (open.value && sum === target.value) return
  clearTimeout(showTimer)
  // From one sum to the next the tooltip follows at once, as tooltips do.
  showTimer = setTimeout(() => show(sum), open.value ? 0 : OPEN_DELAY_MS)
})

useEventListener(document, 'pointerout', (event: PointerEvent) => {
  const sum = sumOf(event.target)
  if (!sum || sumOf(event.relatedTarget) === sum) return
  clearTimeout(showTimer)
  hideTimer = setTimeout(() => {
    open.value = false
  }, 80)
})

onScopeDispose(() => {
  clearTimeout(showTimer)
  clearTimeout(hideTimer)
})
</script>

<template>
  <TooltipRoot :open="open" disable-hoverable-content @update:open="(value: boolean) => (open = value)">
    <TooltipContent v-if="target" :reference="target" side="top" class="min-w-44">
      <MoneyConversions :key="`${currency}:${amount}`" :amount="amount" :currency="currency" />
    </TooltipContent>
  </TooltipRoot>
</template>
