<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Wallet } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { StoreRowLink } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'
import { Card } from '@/shared/ui/card'
import { PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'
import { AllocationPie } from '@/widgets/allocation-pie'
import { NetWorth, type NetWorthSpan, useNetWorth } from '@/widgets/net-worth'

import { groupByPurpose } from '../lib/group-by-purpose'

/**
 * Capital, the accounts screen: the portfolio hero (total, range switch, area chart), what it is made of,
 * and the accounts as list rows grouped by what the money is for. No accounts: one empty state with the first step.
 */
const { t } = useI18n()
const span = ref<NetWorthSpan>('last12')
const worth = useNetWorth(span)
const tree = useStoreTree()
const groups = computed(() => groupByPurpose(tree.data.value ?? []))
</script>

<template>
  <div class="grid gap-4">
    <PageHeader :title="t('stores.title')" :description="t('stores.intro')" />
    <SectionState
      :loading="tree.isPending.value"
      :error="tree.error.value"
      :empty="groups.length === 0"
      :empty-title="t('stores.empty')"
      :empty-description="t('stores.emptyHint')"
      :empty-icon="Wallet"
      skeleton="report"
      @retry="tree.refetch()"
    >
      <div class="grid gap-4">
        <SectionState :loading="worth.loading.value" :error="worth.error.value" :empty="!worth.hasData.value" skeleton="chart" compact @retry="worth.refetch()">
          <NetWorth v-model:span="span" :points="worth.points.value" :currency="worth.currency.value" />
          <AllocationPie :points="worth.points.value" :currency="worth.currency.value" />
        </SectionState>
        <Card v-for="group in groups" :key="group.purpose" class="grid gap-1 p-4 md:p-6" data-tour="stores-list" data-purpose-group>
          <div class="flex items-baseline justify-between gap-3">
            <div class="min-w-0">
              <h3 class="heading-card">{{ t(`purpose.${group.purpose}`) }}</h3>
              <p class="truncate text-xs text-muted-foreground">{{ t(`purpose.${group.purpose}Short`) }}</p>
            </div>
            <span class="shrink-0 text-sm font-medium tabular-nums"><Money :amount="group.total" /></span>
          </div>
          <StoreRowLink v-for="node in group.nodes" :key="node.store.id" :node="node">
            <template #value><Money :amount="node.subtree_base" /></template>
          </StoreRowLink>
        </Card>
      </div>
    </SectionState>
  </div>
</template>
