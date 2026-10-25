<script setup lang="ts">
import { onScopeDispose, ref } from 'vue'
import { useEventListener } from '@vueuse/core'
import { TooltipRoot } from 'reka-ui'

import { TooltipContent } from '@/shared/ui/tooltip'

import { sumOf, sumOfFocus } from '../../lib/money-target'
import { MoneyConversions } from '../money-conversions'

/**
 * The one conversions tooltip for every `<Money>` on screen: a page has
 * hundreds of sums, and a tooltip each was slow to appear and swapped the
 * sum's element on first hover. This host watches the pointer, waits like any
 * tooltip, and shows the conversions next to the sum it rests on — any sum,
 * in text or in a button; a click still opens the card (`MoneyCardHost`).
 * From the keyboard the focus on a button with a sum shows it. A finger has
 * no hover: it taps for the card, or holds a sum in a button.
 */
const OPEN_DELAY_MS = 300
/** Right after a scroll the sums slide under a still pointer: no tooltip then. */
const SCROLL_QUIET_MS = 200

const open = ref(false)
const target = ref<HTMLElement | null>(null)
const amount = ref(0)
const currency = ref('')
const at = ref('')
let showTimer: ReturnType<typeof setTimeout> | undefined
let hideTimer: ReturnType<typeof setTimeout> | undefined
let scrolledAt = 0

function show(sum: HTMLElement) {
  // The card of this very sum is open: it says the same and more.
  if (sum.hasAttribute('data-money-open')) return
  target.value = sum
  amount.value = Number(sum.dataset.moneyAmount)
  currency.value = sum.dataset.moneyCurrency ?? ''
  at.value = sum.dataset.moneyAt ?? ''
  open.value = true
}

function hide() {
  clearTimeout(showTimer)
  clearTimeout(hideTimer)
  open.value = false
}

function schedule(sum: HTMLElement) {
  clearTimeout(hideTimer)
  if (open.value && sum === target.value) return
  clearTimeout(showTimer)
  // From one sum to the next the tooltip follows at once, as tooltips do.
  showTimer = setTimeout(() => show(sum), open.value ? 0 : OPEN_DELAY_MS)
}

useEventListener(document, 'pointerover', (event: PointerEvent) => {
  // A finger has no hover: the tooltip would only flash on a tap.
  if (event.pointerType === 'touch') return
  const sum = sumOf(event.target)
  if (!sum || Date.now() - scrolledAt < SCROLL_QUIET_MS) return
  schedule(sum)
})

useEventListener(document, 'pointerout', (event: PointerEvent) => {
  const sum = sumOf(event.target)
  if (!sum || sumOf(event.relatedTarget) === sum) return
  clearTimeout(showTimer)
  hideTimer = setTimeout(() => {
    open.value = false
  }, 80)
})

// A press, a scroll, Esc: the tooltip is in the way.
useEventListener(document, 'pointerdown', hide, { capture: true })
useEventListener(
  document,
  'scroll',
  () => {
    scrolledAt = Date.now()
    if (open.value || showTimer) hide()
  },
  { capture: true, passive: true },
)
useEventListener(document, 'keydown', (event: KeyboardEvent) => {
  if (event.key === 'Escape' && open.value) hide()
})

// The keyboard: focus on a button or a link with a sum in it (not a click's focus).
useEventListener(document, 'focusin', (event: FocusEvent) => {
  const node = event.target
  if (!(node instanceof Element) || !node.matches(':focus-visible')) return
  const sum = sumOfFocus(node)
  if (sum) schedule(sum)
})
useEventListener(document, 'focusout', (event: FocusEvent) => {
  if (target.value && event.target instanceof Element && event.target.contains(target.value)) hide()
})

onScopeDispose(hide)
</script>

<template>
  <TooltipRoot :open="open" disable-hoverable-content @update:open="(value: boolean) => (open = value)">
    <TooltipContent v-if="target" :reference="target" side="top" :side-offset="8" class="min-w-44">
      <MoneyConversions :key="`${currency}:${amount}:${at}`" :amount="amount" :currency="currency" :at="at" />
    </TooltipContent>
  </TooltipRoot>
</template>
