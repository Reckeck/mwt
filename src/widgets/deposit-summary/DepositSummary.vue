<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { SectionState } from '@/shared/ui/section-state'

/** A savings account, deposit or ПДС at a glance: its rate, what it earned, what is ahead. */
const props = defineProps<{ storeId: string }>()
const { t } = useI18n()

const report = useQuery({
  // no placeholder: another deposit's report
  queryKey: computed(() => ['deposit-report', props.storeId]),
  queryFn: () => call<DepositReport>('get_deposit_report', { storeId: props.storeId }),
})
const data = computed(() => report.data.value ?? null)
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('deposit.title') }}</p>
        <span v-if="data?.rate" class="text-lg font-semibold tabular-nums">{{ data.rate }}%</span>
      </div>
    </template>
    <Skeleton v-if="report.isPending.value" class="h-24 w-full" />
    <SectionState v-else-if="report.error.value" :error="report.error.value" compact @retry="report.refetch()" />
    <div v-else-if="data && !data.rate" class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('deposit.noTerms') }}</p>
      <RouterLink
        :to="{ name: 'wallet-settings', params: { id: props.storeId }, query: { tab: 'deposit' } }"
        :class="buttonVariants({ variant: 'outline', size: 'sm' })"
      >
        {{ t('deposit.setTerms') }}
      </RouterLink>
    </div>
    <div v-else-if="data" class="grid gap-4">
      <dl class="grid grid-cols-2 gap-x-4 gap-y-1.5 text-sm @mid:grid-cols-4">
        <div class="grid gap-0.5">
          <dt class="text-xs text-muted-foreground">{{ t('deposit.accruedMonth') }}</dt>
          <dd class="font-medium"><Money :amount="data.accrued_month" :currency="data.currency" /></dd>
        </div>
        <div class="grid gap-0.5">
          <dt class="text-xs text-muted-foreground">{{ t('deposit.accruedYear') }}</dt>
          <dd class="font-medium"><Money :amount="data.accrued_year" :currency="data.currency" /></dd>
        </div>
        <div class="grid gap-0.5">
          <dt class="text-xs text-muted-foreground">{{ t('deposit.postedYear') }}</dt>
          <dd class="font-medium"><Money :amount="data.posted_year" :currency="data.currency" /></dd>
        </div>
        <div class="grid gap-0.5">
          <dt class="text-xs text-muted-foreground">{{ t('deposit.projectedYear') }}</dt>
          <dd class="font-medium text-success">
            +<Money :amount="data.projected_year" :currency="data.currency" />
          </dd>
        </div>
      </dl>
      <p v-if="data.rate_since" class="text-xs text-muted-foreground">{{ t('deposit.rateSince', { date: formatDate(data.rate_since) }) }}</p>
      <p v-if="data.matures_at" class="text-sm">
        {{ t('deposit.matures', { date: formatDate(data.matures_at) }) }}
        <template v-if="data.at_maturity">
          — <Money :amount="data.at_maturity" :currency="data.currency" class="font-medium" />
        </template>
      </p>

      <div v-if="data.pds" class="grid gap-2 rounded-lg border border-border p-3 text-sm">
        <p class="font-medium">{{ t('deposit.pds.title') }} · {{ data.pds.year }}</p>
        <dl class="grid grid-cols-2 gap-x-4 gap-y-1">
          <dt class="text-muted-foreground">{{ t('deposit.pds.contributions') }}</dt>
          <dd class="text-right"><Money :amount="data.pds.contributions" currency="rub" /></dd>
          <dt class="text-muted-foreground">{{ t('deposit.pds.cofinancing', { ratio: data.pds.ratio }) }}</dt>
          <dd class="text-right font-medium text-success">
            <Money :amount="data.pds.cofinancing" currency="rub" />
          </dd>
          <template v-if="Number(data.pds.to_full_cofinancing) > 0">
            <dt class="text-muted-foreground">{{ t('deposit.pds.toFull') }}</dt>
            <dd class="text-right"><Money :amount="data.pds.to_full_cofinancing" currency="rub" /></dd>
          </template>
          <dt class="text-muted-foreground">{{ t('deposit.pds.deduction') }}</dt>
          <dd class="text-right"><Money :amount="data.pds.deduction" currency="rub" /></dd>
        </dl>
        <p class="text-xs text-muted-foreground">
          {{ t('deposit.pds.due', { date: formatDate(data.pds.cofinancing_due), end: formatDate(data.pds.ends_at) }) }}
        </p>
      </div>
    </div>
  </Card>
</template>
