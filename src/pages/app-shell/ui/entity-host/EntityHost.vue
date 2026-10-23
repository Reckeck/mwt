<script setup lang="ts">
import { BudgetDialog } from '@/features/budget-edit'
import { GoalDialog } from '@/features/goal-edit'
import { PriceAlertDialog } from '@/features/price-alert-edit'
import { EntityRelations, EntitySheet } from '@/widgets/entity-sheet'
import { WalletGlance } from '@/widgets/quick-glance'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'

import { useEntityHost } from './use-entity-host'

/** Every entity card of the app, drawn from the address (`?open=goal:g1`). */
const host = useEntityHost()
const { txOpen, cardOpen, goalOpen, budgetOpen, alertOpen } = host
</script>

<template>
  <TransactionDetailsSheet v-model:open="txOpen" :tx="host.tx.value">
    <template #side="{ side, label }">
      <WalletGlance v-if="'store' in side" :store-id="side.store" :name="label" />
      <template v-else>{{ label }}</template>
    </template>
    <template #relations>
      <EntityRelations v-if="host.current.value?.groups.length" :groups="host.current.value.groups" />
    </template>
  </TransactionDetailsSheet>
  <EntitySheet
    v-model:open="cardOpen"
    :card="host.current.value"
    :loading="host.card.isFetching.value"
    :error="host.card.error.value"
    :can-go-back="host.canGoBack.value"
    @back="host.back"
  />

  <GoalDialog v-model:open="goalOpen" :store-id="host.request.value?.target.id" />
  <BudgetDialog v-model:open="budgetOpen" :category="host.request.value?.target.id" />
  <PriceAlertDialog v-model:open="alertOpen" :preset="host.request.value ? { symbol: host.request.value.title, price: '' } : null" />
</template>
