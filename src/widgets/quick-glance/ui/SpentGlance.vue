<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { drillTo } from '@/shared/lib/drill'
import { useTxContext } from '@/shared/lib/tx-context'
import { Button } from '@/shared/ui/button'
import { GLANCE_TRIGGER_CLASS, Glance } from '@/shared/ui/glance'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { Separator } from '@/shared/ui/separator'

import { glancePayments } from '../lib/payments'
import { useSpentGlance } from '../model/use-spent-glance'
import type { SpentGlanceProps } from '../quick-glance.types'

/**
 * «Потрачено в октябре 36 $» opened on the spot (Paths 1 · 2): against the month
 * before, the categories' shares and the last three payments; then «Задать лимит»
 * or «Открыть в Тратах →». A category opens its operations with the way back.
 */
const props = defineProps<SpentGlanceProps>()
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const context = useTxContext()
const categories = useCategories()

const open = ref(false)
const glance = useSpentGlance(() => props.month, open)

const monthName = computed(() => t(`period.in.${props.month.slice(5, 7)}`))
const monthBefore = computed(() => {
  const number = Number(props.month.slice(5, 7))
  return t(`period.in.${String(number === 1 ? 12 : number - 1).padStart(2, '0')}`)
})
const title = computed(() => t('home.figures.spent', { when: monthName.value }))
const back = computed(() => ({ to: route.fullPath, label: props.backLabel }))
const label = (key: string) => (key ? categoryKeyLabel(key, categories.data.value, t) : t('money.categories.none'))
const categoryTo = (key: string) => drillTo({ from: props.month, category: key, kinds: ['payment'], back: back.value })

const payments = computed(() => glancePayments(glance.payments.value, { side: context.sideLabel, category: label }))

function setLimit() {
  open.value = false
  void router.push({ name: 'money-plan', query: { new: 'limit' } })
}
</script>

<template>
  <Glance
    v-model:open="open"
    :title="title"
    :open-to="{ name: 'money', query: { from: props.month, to: props.month } }"
    :open-label="t('glance.spent.open')"
    :trigger-class="props.triggerClass"
    kind="spent"
  >
    <slot />
    <template #body>
      <div class="flex flex-col gap-0.5">
        <span class="text-xs text-muted-foreground">{{ title }}</span>
        <span class="text-lg font-semibold tabular-nums"><Money :amount="glance.spent.value" :currency="glance.currency.value" :decimals="0" /></span>
        <span class="text-xs text-muted-foreground">
          <i18n-t keypath="glance.spent.before">
            <template #month>{{ monthBefore }}</template>
            <template #sum><Money :amount="glance.spentBefore.value" :currency="glance.currency.value" :decimals="0" /></template>
          </i18n-t>
          ·
          <i18n-t v-if="glance.limit.value" keypath="glance.spent.limit">
            <template #limit>
              <Money :amount="glance.limit.value.budget.limit" :currency="glance.limit.value.budget.currency" :decimals="0" />
            </template>
          </i18n-t>
          <template v-else>{{ t('glance.spent.noLimit') }}</template>
        </span>
      </div>
      <div v-for="share in glance.shares.value.slice(0, 4)" :key="share.key" class="flex flex-col gap-1">
        <div class="flex items-baseline gap-2">
          <span class="min-w-0 flex-1 truncate">
            <RouterLink data-glance-category :to="categoryTo(share.key)" :class="GLANCE_TRIGGER_CLASS" @click="open = false">{{ label(share.key) }}</RouterLink>
          </span>
          <b class="tabular-nums"><Money :amount="share.total" :currency="glance.currency.value" :decimals="0" /></b>
          <span class="w-10 text-right text-muted-foreground tabular-nums">{{ share.pct }} %</span>
        </div>
        <ProgressBar :value="share.pct" size="sm" />
      </div>
      <template v-if="payments.length">
        <Separator />
        <span class="text-xs text-muted-foreground">{{ t('glance.spent.last') }}</span>
        <div v-for="payment in payments" :key="payment.id" class="flex items-baseline gap-2">
          <span class="min-w-0 flex-1 truncate">{{ payment.name }} <span class="text-muted-foreground">· {{ payment.day }}</span></span>
          <b class="tabular-nums"><Money :amount="payment.amount" :currency="payment.currency" :at="payment.at" :decimals="0" /></b>
        </div>
      </template>
    </template>
    <template #actions>
      <Button variant="outline" size="sm" data-glance-limit @click="setLimit">{{ t('home.figures.setLimitLong') }}</Button>
    </template>
  </Glance>
</template>
