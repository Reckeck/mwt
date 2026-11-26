<script setup lang="ts">
import { BudgetDialog } from '@/features/budget-edit'
import { GoalDialog } from '@/features/goal-edit'
import { PriceAlertDialog } from '@/features/price-alert-edit'
import { EntitySheet } from '@/widgets/entity-sheet'
import { GoalDetailsSheet } from '@/widgets/goal-details'
import { WalletGlance } from '@/widgets/quick-glance'

import { useEntityHost } from './use-entity-host'

/** Every entity card of the app, drawn from the address (`?open=goal:g1`). */
const host = useEntityHost()
const { goalCardOpen, cardOpen, goalOpen, budgetOpen, alertOpen } = host
</script>

<template>
  <GoalDetailsSheet
    v-model:open="goalCardOpen"
    :goal-id="host.top.value?.kind === 'goal' ? host.top.value.id : null"
    :groups="host.current.value?.groups"
  >
    <template #wallet="{ storeId, label }"><WalletGlance :store-id="storeId" :name="label" /></template>
  </GoalDetailsSheet>
  <EntitySheet
    v-model:open="cardOpen"
    :card="host.current.value"
    :loading="host.card.isFetching.value"
    :error="host.card.error.value"
  />

  <GoalDialog v-model:open="goalOpen" :store-id="host.request.value?.target.id" />
  <BudgetDialog v-model:open="budgetOpen" :category="host.request.value?.target.id" />
  <PriceAlertDialog v-model:open="alertOpen" :preset="host.request.value ? { symbol: host.request.value.title, price: '' } : null" />
</template>
