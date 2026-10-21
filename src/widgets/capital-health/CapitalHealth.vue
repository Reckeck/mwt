<script setup lang="ts">
import { computed } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY, useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { SectionState } from '@/shared/ui/section-state'

import DebtsCard from './ui/DebtsCard.vue'
import FreedomCard from './ui/FreedomCard.vue'
import FundCard from './ui/FundCard.vue'
import HealthLight from './ui/HealthLight.vue'
import SavingsCard from './ui/SavingsCard.vue'

/** The capital checkup: the traffic light, then the fund, debts, savings rate and freedom index. */
const report = useQuery({
  queryKey: CAPITAL_HEALTH_KEY,
  queryFn: () => call<HealthReport>('capital_health'),
  placeholderData: keepPreviousData,
})
const tree = useStoreTree()
const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })

/** Names for the links: wallets and goals share one map, their ids never collide. */
const names = computed<Record<string, string>>(() => ({
  ...Object.fromEntries(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name])),
  ...Object.fromEntries((goals.data.value ?? []).map((goal) => [goal.id, goal.name])),
}))
</script>

<template>
  <SectionState
    :loading="report.isPending.value"
    :error="report.error.value"
    skeleton="report"
    @retry="report.refetch()"
  >
    <div v-if="report.data.value" class="grid gap-4">
      <HealthLight :report="report.data.value" :names="names" />
      <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <FundCard :fund="report.data.value.emergency" :names="names" />
        <DebtsCard :debts="report.data.value.debts" :names="names" />
        <SavingsCard :report="report.data.value" />
        <FreedomCard
          :passive="report.data.value.passive"
          :rate="report.data.value.freedom_rate"
          :key-rate="report.data.value.debts.key_rate"
        />
      </div>
    </div>
  </SectionState>
</template>
