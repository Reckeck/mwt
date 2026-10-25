<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { formatPct } from '@/shared/lib/format'
import { type PeriodPreset, presetOf, presetRange } from '@/shared/lib/period'
import { Button } from '@/shared/ui/button'

import { Money } from '@/entities/currency'
import { wealthChange } from '@/entities/report'

/**
 * «Капитал 42 052,50 $» with what the month and the period changed, and the
 * spans one compares over (Money 2 · 12). The chart goes in the slot.
 */
const props = defineProps<{ points: NetworthPoint[]; currency: string; range: { from: string; to: string } }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const total = computed(() => props.points.at(-1)?.net ?? '0')
const change = computed(() => wealthChange(props.points))
const pct = (value: number) => formatPct(value, { signed: true, decimals: 1 })
const tone = computed(() => ((change.value?.month ?? 0) < 0 ? 'text-destructive' : 'text-success'))

const SPANS: PeriodPreset[] = ['last3', 'last6', 'last12', 'all']
const span = computed(() => presetOf(props.range))
/** The default span (a year) clears the address, like the period picker does. */
function pick(preset: PeriodPreset) {
  const range = presetRange(preset)
  const query = { ...route.query, from: preset === 'last12' ? undefined : range.from, to: preset === 'last12' ? undefined : range.to }
  void router.replace({ query })
}
</script>

<template>
  <section class="grid min-w-0 gap-2 rounded-xl border bg-card p-4 shadow-sm" data-tour="wealth-head">
    <span class="text-xs text-muted-foreground">{{ t('money.wealth.title') }}</span>
    <p class="text-2xl font-bold tabular-nums">
      <Money :amount="total" :currency="props.currency" />
    </p>
    <p v-if="change" class="text-xs tabular-nums" :class="tone">
      <span>
        <Money :amount="change.month" :currency="props.currency" :decimals="0" signed />
        <template v-if="change.monthPct !== null"> ({{ pct(change.monthPct) }})</template>
        {{ t('money.wealth.perMonth') }}
      </span>
      <span class="max-md:hidden">
        ·
        <Money :amount="change.period" :currency="props.currency" :decimals="0" signed />
        {{ t('money.wealth.perMonths', { count: change.months }) }}
      </span>
    </p>
    <fieldset class="flex flex-wrap gap-1.5 max-md:hidden">
      <legend class="sr-only">{{ t('money.wealth.span') }}</legend>
      <Button
        v-for="entry in SPANS"
        :key="entry"
        size="sm"
        :variant="span === entry ? 'default' : 'outline'"
        :aria-pressed="span === entry"
        data-wealth-span
        @click="pick(entry)"
      >
        {{ t(`money.wealth.spans.${entry}`) }}
      </Button>
    </fieldset>
    <slot />
  </section>
</template>
