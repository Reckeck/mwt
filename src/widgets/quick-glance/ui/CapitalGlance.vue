<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { formatPct } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Glance } from '@/shared/ui/glance'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { Separator } from '@/shared/ui/separator'
import { Sparkline } from '@/shared/ui/sparkline'

import { useCapitalGlance } from '../model/use-capital-glance'
import type { CapitalGlanceProps } from '../quick-glance.types'

/**
 * «Капитал 42 052,50 $» opened on the spot (Paths 1 · 3): the month's change, the
 * year's line, what it is made of and one sentence on what moved it; then
 * «Кошельки →» or «Открыть Капитал →».
 */
const props = defineProps<CapitalGlanceProps>()
const { t } = useI18n()
const router = useRouter()

const open = ref(false)
const glance = useCapitalGlance(open)

const change = computed(() => {
  const value = glance.change.value
  if (!value) return null
  const pct = value.monthPct === null ? '' : ` (${formatPct(value.monthPct, { signed: true, decimals: 1 })})`
  return { month: value.month, tail: `${pct} ${t('money.wealth.perMonth')}` }
})
const tone = computed(() => ((glance.change.value?.month ?? 0) < 0 ? 'text-destructive' : 'text-success'))
/** «Выросло в основном за счёт Крипты: +312 $ за месяц». */
const why = computed(() => {
  const driver = glance.driver.value
  if (!driver) return null
  return {
    key: driver.up ? 'glance.capital.grew' : 'glance.capital.fell',
    group: t(`money.wealth.group.${driver.group}Of`),
    delta: driver.delta,
  }
})

function toWallets() {
  open.value = false
  void router.push({ name: 'stores' })
}
</script>

<template>
  <Glance overlay="glance-capital"
    v-model:open="open"
    :title="t('money.wealth.title')"
    :open-to="{ name: 'money-wealth' }"
    :open-label="t('glance.capital.open')"
    :trigger-class="props.triggerClass"
    kind="capital"
  >
    <slot />
    <template #body>
      <div class="flex flex-col gap-0.5">
        <span class="text-xs text-muted-foreground">{{ t('money.wealth.title') }}</span>
        <span class="text-lg font-semibold tabular-nums"><Money :amount="glance.total.value" :currency="glance.currency.value" /></span>
        <span v-if="change" class="text-xs tabular-nums" :class="tone">
          <Money :amount="change.month" :currency="glance.currency.value" :decimals="0" signed />{{ change.tail }}
        </span>
      </div>
      <Sparkline v-if="glance.line.value.length > 1" :values="glance.line.value" area class="h-9 text-primary" />
      <div v-for="row in glance.rows.value" :key="row.group" class="flex flex-col gap-1">
        <div class="flex items-baseline gap-2">
          <span class="min-w-0 flex-1 truncate">{{ t(`money.wealth.group.${row.group}`) }}</span>
          <b class="tabular-nums" :class="row.value < 0 && 'text-destructive'"><Money :amount="row.value" :currency="glance.currency.value" /></b>
          <span class="w-10 text-right text-muted-foreground tabular-nums">{{ row.share === null ? '' : `${row.share} %` }}</span>
        </div>
        <ProgressBar v-if="row.share !== null" :value="row.share" size="sm" />
      </div>
      <template v-if="why">
        <Separator />
        <p class="text-xs text-muted-foreground" data-glance-why>
          <i18n-t :keypath="why.key">
            <template #group>{{ why.group }}</template>
            <template #delta><Money :amount="why.delta" :currency="glance.currency.value" :decimals="0" signed /></template>
          </i18n-t>
        </p>
      </template>
    </template>
    <template #actions>
      <Button variant="link" size="sm" class="px-0" data-glance-wallets @click="toWallets">{{ t('glance.capital.wallets') }}</Button>
    </template>
  </Glance>
</template>
