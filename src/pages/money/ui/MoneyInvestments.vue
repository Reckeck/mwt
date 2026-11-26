<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { TrendingUp } from 'lucide-vue-next'

import { positionTotals, useCryptoPositions } from '@/entities/crypto-position'
import { Money } from '@/entities/currency'
import { StoreRowLink } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'
import { withoutLoyalty } from '@/shared/lib/store-tree'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'

/**
 * Money · Investments: the accounts kept for investing with their sum, and the DeFi positions of the followed
 * addresses as one card that opens the positions list. Neither a tax nor a foreign-account report lives here.
 */
const { t } = useI18n()
const router = useRouter()
const tree = useStoreTree()
const positions = useCryptoPositions()

const nodes = computed(() =>
  withoutLoyalty(tree.data.value ?? []).filter((node) => !node.store.archived && node.store.purpose === 'investment'),
)
const total = computed(() => nodes.value.reduce((sum, node) => sum + Number(node.subtree_base), 0))
const defi = computed(() => positionTotals(positions.data.value ?? []))
const hasDefi = computed(() => (positions.data.value ?? []).length > 0)
</script>

<template>
  <div class="grid gap-4">
    <PageHeader :title="t('money.invest.title')" />
    <SectionState
      :loading="tree.isPending.value"
      :error="tree.error.value"
      :empty="nodes.length === 0 && !hasDefi"
      :empty-title="t('money.invest.empty')"
      :empty-description="t('money.invest.emptyHint')"
      :empty-icon="TrendingUp"
      skeleton="report"
      @retry="tree.refetch()"
    >
      <div class="grid gap-4">
        <Card v-if="nodes.length" class="grid gap-1 p-4 md:p-6" data-tour="invest-figures" data-invest-accounts>
          <div class="flex items-baseline justify-between gap-3">
            <h3 class="heading-card">{{ t('money.invest.accounts') }}</h3>
            <span class="shrink-0 text-sm font-medium tabular-nums"><Money :amount="total" /></span>
          </div>
          <StoreRowLink v-for="node in nodes" :key="node.store.id" :node="node">
            <template #value><Money :amount="node.subtree_base" /></template>
          </StoreRowLink>
        </Card>
        <Card v-if="hasDefi" class="flex flex-wrap items-center justify-between gap-3 p-4 md:p-6" data-tour="invest-table" data-invest-defi>
          <div class="min-w-0">
            <h3 class="heading-card">{{ t('cryptoTrack.positions.title') }}</h3>
            <p class="text-sm tabular-nums"><Money :amount="defi.value" currency="USD" /></p>
            <p v-if="defi.missing" class="text-xs text-muted-foreground">
              {{ t('cryptoTrack.positions.summary.missing', { count: defi.missing }) }}
            </p>
          </div>
          <Button variant="outline" data-invest-defi-open @click="router.push({ name: 'crypto-positions' })">
            {{ t('money.invest.defiOpen') }}
          </Button>
        </Card>
      </div>
    </SectionState>
  </div>
</template>
