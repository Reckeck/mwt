<script setup lang="ts">
import { ref, watch } from 'vue'
import { useEventListener } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { notify } from '@/shared/lib/notify'
import { useIsPhone } from '@/shared/lib/phone'
import { settingsQuery } from '@/shared/lib/settings-link'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'
import { Kbd } from '@/shared/ui/kbd'
import { Popover, PopoverAnchor, PopoverContent } from '@/shared/ui/popover'

import { type SumData, isPressableSum, sumData, sumOf } from '../../lib/money-target'
import { MoneyCard } from '../money-card'

/**
 * The one sum card for every `<Money>` on screen (Currency-1 · 5): a click on a
 * sum — a tap on a phone — shows it in the base currency at the operation's
 * rate and today's, and in the favourites. A card by the sum on a computer, a
 * sheet on a phone. C copies the number, Esc closes, «Все курсы» opens the
 * rates in Настройки › Деньги. A sum inside a button or a link keeps its click
 * and the hover tooltip (`MoneyTooltipHost`).
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const phone = useIsPhone()

const open = ref(false)
const target = ref<HTMLElement | null>(null)
const sum = ref<SumData | null>(null)
/** The click that closed the card by pressing its own sum must not open it again. */
let closed = { at: 0, sum: null as HTMLElement | null }

useEventListener(
  document,
  'click',
  (event: MouseEvent) => {
    const node = sumOf(event.target)
    if (!node || !isPressableSum(node)) return
    // The row under the sum must not open its operation as well.
    event.stopPropagation()
    if (closed.sum === node && Date.now() - closed.at < 400) return
    target.value?.removeAttribute('data-money-open')
    target.value = node
    sum.value = sumData(node)
    node.setAttribute('data-money-open', '')
    open.value = true
  },
  { capture: true },
)

watch(open, (value) => {
  if (value) return
  closed = { at: Date.now(), sum: target.value }
  target.value?.removeAttribute('data-money-open')
})

async function copy() {
  if (!sum.value) return
  await navigator.clipboard?.writeText(String(Math.abs(sum.value.amount))).catch(() => undefined)
  notify.success('ui.copied')
}

function allRates() {
  open.value = false
  void router.push({ query: settingsQuery(route.query, 'accounting', 'rates') })
}

/** The card takes focus itself, not its first button: C and Esc work at once, no ring on «Все курсы». */
function focusCard(event: Event) {
  event.preventDefault()
  ;(event.target as HTMLElement | null)?.focus()
}

function onKey(event: KeyboardEvent) {
  if (event.key.toLowerCase() !== 'c' && event.code !== 'KeyC') return
  if (event.ctrlKey || event.metaKey || event.altKey) return
  event.preventDefault()
  void copy()
}
</script>

<template>
  <Popover v-if="!phone" v-model:open="open">
    <PopoverAnchor v-if="target" :key="sum ? `${sum.currency}:${sum.amount}:${sum.at}` : ''" :reference="target" />
    <PopoverContent
      v-if="sum"
      align="end"
      :side-offset="8"
      class="flex w-[min(340px,calc(100vw-2rem))] flex-col gap-0 rounded-[10px] p-0"
      data-money-card-host
      @open-auto-focus="focusCard"
      @keydown="onKey"
    >
      <div class="flex flex-col gap-2.5 p-3.5">
        <MoneyCard :key="`${sum.currency}:${sum.amount}:${sum.at}`" v-bind="sum" />
        <p class="flex gap-3 text-xs text-muted-foreground">
          <span><Kbd>C</Kbd> {{ t('moneyCard.copy') }}</span>
          <span><Kbd>Esc</Kbd> {{ t('glance.close') }}</span>
        </p>
      </div>
      <div class="flex border-t border-border px-3.5 py-2.5">
        <Button variant="link" size="sm" class="h-auto px-0 text-sm" data-money-card-rates @click="allRates">
          {{ t('moneyCard.allRates') }} →
        </Button>
      </div>
    </PopoverContent>
  </Popover>
  <Drawer v-else v-model:open="open">
    <DrawerContent v-if="sum" class="gap-3 px-4 pt-2 pb-5" data-money-card-host @open-auto-focus="focusCard">
      <DrawerTitle class="sr-only">{{ t('moneyCard.title') }}</DrawerTitle>
      <MoneyCard :key="`${sum.currency}:${sum.amount}:${sum.at}`" v-bind="sum" />
      <div class="grid grid-cols-[auto_1fr] gap-2 [&_button]:h-11">
        <Button variant="outline" @click="copy">{{ t('moneyCard.copyButton') }}</Button>
        <Button data-money-card-rates @click="allRates">{{ t('moneyCard.allRates') }}</Button>
      </div>
    </DrawerContent>
  </Drawer>
</template>
