<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { ArrowRight } from 'lucide-vue-next'

import { assetClassName } from '@/entities/asset-class'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { TOPUP_ADVICE_KEY } from '@/shared/api/queries'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

/**
 * «Куда отложить» (plan H4): last month's savings split by the target shares.
 * S — the sum; M — the baskets that get money; L — every basket.
 */
const { t } = useI18n()
const size = useWidgetSize()

const advice = useQuery({
  queryKey: [...TOPUP_ADVICE_KEY, null],
  queryFn: () => call<TopupAdvice>('topup_advice', { amount: null }),
  placeholderData: keepPreviousData,
})

const rows = computed(() => {
  const all = advice.data.value?.rows ?? []
  return size.value === 'l' ? all : all.filter((row) => Number(row.add) > 0)
})
const empty = computed(() => !advice.data.value || Number(advice.data.value.amount) <= 0)
</script>

<template>
  <Card size="sm">
    <SectionState
      :loading="advice.isPending.value"
      :error="advice.error.value"
      :empty="empty"
      :empty-title="t('dashboard.topup.empty')"
      skeleton="list"
      :rows="3"
      compact
      @retry="advice.refetch()"
    >
      <div v-if="advice.data.value" class="grid gap-2" data-tour="dash-topup">
        <p class="truncate text-xs text-muted-foreground">
          {{ advice.data.value.from_savings ? t('dashboard.topup.fromSavings') : t('dashboard.widget.topupAdvisor') }}
        </p>
        <p class="figure truncate">
          <Money :amount="advice.data.value.amount" :currency="advice.data.value.currency" :decimals="0" :convert="false" />
        </p>
        <ul v-if="size !== 's'" class="grid gap-1 text-sm">
          <li v-for="row in rows" :key="row.class.id" class="flex min-w-0 items-baseline justify-between gap-2">
            <span class="truncate">{{ assetClassName(row.class, t) }}</span>
            <span class="shrink-0 font-medium tabular-nums text-success">
              + <Money :amount="row.add" :currency="advice.data.value.currency" :decimals="0" :convert="false" />
            </span>
          </li>
        </ul>
        <Button v-if="size !== 's'" variant="link" size="sm" class="h-auto self-start p-0" as-child>
          <RouterLink :to="{ name: 'money-investments' }">
            {{ t('dashboard.topup.open') }}
            <ArrowRight class="size-3.5" />
          </RouterLink>
        </Button>
      </div>
    </SectionState>
  </Card>
</template>
