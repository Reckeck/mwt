<script setup lang="ts">
import { BudgetSettings } from '@/widgets/budget-settings'
import { CategorizationRules } from '@/widgets/categorization-rules'
import { CategoryManager } from '@/widgets/category-manager'
import { EnrichmentSettings } from '@/widgets/enrichment-settings'
import { InflationSettings } from '@/widgets/inflation-settings'
import { QuickAddSettings } from '@/widgets/quick-add-settings'
import { VaultDevTools } from '@/widgets/vault-dev-tools'
import { VaultJsonEditor } from '@/widgets/vault-json-editor'
import { VaultSettings } from '@/widgets/vault-settings'

const isDev = import.meta.env.DEV
</script>

<template>
  <div class="mx-auto grid max-w-3xl gap-4">
    <VaultSettings />
    <BudgetSettings />
    <CategoryManager />
    <CategorizationRules />
    <InflationSettings />
    <EnrichmentSettings />
    <QuickAddSettings />
    <VaultDevTools v-if="isDev">
      <template #viewer>
        <VaultJsonEditor />
      </template>
    </VaultDevTools>
  </div>
</template>
