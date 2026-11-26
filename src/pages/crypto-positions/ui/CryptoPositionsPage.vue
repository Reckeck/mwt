<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Layers } from 'lucide-vue-next'

import { CryptoPositionRow, positionTotals, protocolsOf, useCryptoPositions } from '@/entities/crypto-position'
import { Money } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'
import { SwitchRow } from '@/shared/ui/switch-row'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { TradingChart } from '@/widgets/trading-chart'

import PositionsAddresses from './PositionsAddresses.vue'

/**
 * Capital · DeFi positions: what the followed addresses hold inside protocols. The worth of the open ones on top,
 * a protocol filter and the closed ones behind a switch. Read-only: a position is read from the network, never typed in.
 */
const { t } = useI18n()
const ALL = 'all'
const positions = useCryptoPositions()
const protocol = ref(ALL)
const showClosed = ref(false)

const all = computed(() => positions.data.value ?? [])
const protocols = computed(() => protocolsOf(all.value))
const totals = computed(() => positionTotals(all.value))
const shown = computed(() =>
  all.value.filter(
    (position) => (protocol.value === ALL || position.protocol === protocol.value) && (showClosed.value || position.status === 'open'),
  ),
)

const pickProtocol = (value: unknown) => {
  if (typeof value === 'string' && value) protocol.value = value
}
</script>

<template>
  <div class="grid gap-4">
    <PageHeader :title="t('cryptoTrack.positions.title')" :description="t('cryptoTrack.positions.pageHint')" />
    <SectionState
      :loading="positions.isPending.value"
      :error="positions.error.value"
      :empty="all.length === 0"
      :empty-title="t('cryptoTrack.positions.empty.title')"
      :empty-description="t('cryptoTrack.positions.empty.description')"
      :empty-icon="Layers"
      skeleton="list"
      @retry="positions.refetch()"
    >
      <div class="grid gap-4">
        <Card class="grid gap-1 p-4 md:p-6" data-tour="defi-net" data-positions-total>
          <p class="text-sm text-muted-foreground">{{ t('cryptoTrack.positions.value') }}</p>
          <p class="text-3xl font-semibold tabular-nums"><Money :amount="totals.value" currency="USD" /></p>
          <p v-if="totals.missing" class="text-xs text-muted-foreground" data-positions-missing>
            {{ t('cryptoTrack.positions.summary.missing', { count: totals.missing }) }}
          </p>
        </Card>
        <div class="flex flex-wrap items-center justify-between gap-3">
          <ToggleGroup
            type="single"
            variant="range"
            :spacing="1"
            :model-value="protocol"
            :aria-label="t('cryptoTrack.positions.filter')"
            data-protocol-filter
            @update:model-value="pickProtocol"
          >
            <ToggleGroupItem :value="ALL" data-protocol="all">{{ t('cryptoTrack.positions.allProtocols') }}</ToggleGroupItem>
            <ToggleGroupItem v-for="name in protocols" :key="name" :value="name" :data-protocol="name">{{ name }}</ToggleGroupItem>
          </ToggleGroup>
          <SwitchRow
            id="positions-show-closed"
            v-model="showClosed"
            plain
            :label="t('cryptoTrack.positions.showClosed')"
            data-show-closed
          />
        </div>
        <Card class="grid gap-1 p-4 md:p-6" data-tour="defi-list">
          <p v-if="shown.length === 0" class="py-4 text-sm text-muted-foreground" data-positions-none>
            {{ t('cryptoTrack.positions.noneMatch') }}
          </p>
          <CryptoPositionRow v-for="position in shown" :key="position.id" :position="position">
            <template #value><Money :amount="position.value_usd" currency="USD" /></template>
            <template #net><Money :amount="position.net_usd" currency="USD" signed colored /></template>
          </CryptoPositionRow>
        </Card>
        <p class="text-xs text-muted-foreground" data-tour="defi-fees">{{ t('cryptoTrack.positions.legend') }}</p>
      </div>
    </SectionState>
    <PositionsAddresses />
    <TradingChart />
  </div>
</template>
