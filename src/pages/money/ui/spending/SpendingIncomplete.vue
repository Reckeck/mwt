<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Info } from 'lucide-vue-next'

import { drillTo } from '@/shared/lib/drill'
import { dateFormat, formatList, pluralForm } from '@/shared/lib/format'
import { settingsQuery } from '@/shared/lib/settings-link'
import { Button } from '@/shared/ui/button'

/**
 * «Итог неполный» (Currency-1 · 7): spending without a rate is kept, but left
 * out of the sums; the banner says how much, when and in what, and offers to
 * see those operations or to enter the rate.
 */
const props = defineProps<{ unpriced: UnpricedSpending; from: string; to: string }>()

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()

/** «3 и 5 окт.»: the days grouped by month, each month named once. */
const days = computed(() => {
  const months = new Map<string, string[]>()
  for (const day of props.unpriced.days) months.set(day.slice(0, 7), [...(months.get(day.slice(0, 7)) ?? []), day])
  const month = dateFormat({ month: 'short' })
  return formatList(
    [...months.values()].map((list) =>
      t('money.incomplete.days', {
        days: formatList(list.map((day) => String(Number(day.slice(8, 10))))),
        month: month.format(new Date(`${list[0]}T12:00:00`)),
      }),
    ),
  )
})
const form = computed(() => pluralForm(props.unpriced.count, locale.value))
const detail = computed(() =>
  t(`money.incomplete.detail.${form.value}`, {
    count: props.unpriced.count,
    days: days.value,
    currencies: props.unpriced.currencies.join(', '),
  }),
)
const short = computed(() => t(`money.incomplete.short.${form.value}`, { count: props.unpriced.count }))

function show() {
  const back = { to: route.fullPath, label: t('money.tab.spending') }
  void router.push(drillTo({ from: props.from, to: props.to, kinds: ['payment'], unpriced: true, back }))
}
function enterRate() {
  void router.push({ query: settingsQuery(route.query, 'money', 'rates') })
}
</script>

<template>
  <section
    class="flex items-center gap-2.5 rounded-xl border border-warning bg-card px-3.5 py-2.5 text-sm shadow-sm max-md:flex-col max-md:items-stretch max-md:gap-2 max-md:px-3"
    data-spending-incomplete
  >
    <Info class="size-4 shrink-0 text-warning max-md:hidden" aria-hidden="true" />
    <p class="min-w-0 flex-1 max-md:hidden">{{ t('money.incomplete.title') }} {{ detail }}</p>
    <p class="md:hidden"><span class="text-warning">{{ t('money.incomplete.title') }}</span> {{ short }}</p>
    <div class="flex gap-2 max-md:[&>button]:h-10 max-md:[&>button]:flex-1">
      <Button variant="outline" size="sm" data-spending-incomplete-show @click="show">{{ t('money.incomplete.show') }}</Button>
      <Button variant="outline" size="sm" @click="enterRate">{{ t('money.incomplete.setRate') }}</Button>
    </div>
  </section>
</template>
