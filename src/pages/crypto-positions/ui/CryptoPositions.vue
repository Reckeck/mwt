<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { Layers, RefreshCw } from 'lucide-vue-next'

import { ALL_PROTOCOLS, familyLabel, matchesProtocol, protocolsOf, useCryptoPositions } from '@/entities/crypto-position'
import { useIsPhone } from '@/shared/lib/phone'
import { CryptoPositionDetail } from './crypto-position-detail'
import { CryptoPositionSummary } from './crypto-position-summary'
import { CryptoPositionTable } from './crypto-position-table'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { EmptyState } from '@/shared/ui/empty-state'
import { PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

/**
 * What the followed addresses hold inside protocols (spec D5). Read only: the adapters find the positions and
 * the engine computes every figure; the page composes the summary, the filters, the list and the detail.
 * The filter, the closed switch and the open position live in the address, so «Back» and a reload keep them.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const positions = useCryptoPositions()
const phone = useIsPhone()

const protocol = computed(() => (typeof route.query.protocol === 'string' ? route.query.protocol : ALL_PROTOCOLS))
const showClosed = computed(() => route.query.closed === '1')
const openId = computed(() => (typeof route.query.position === 'string' ? route.query.position : undefined))

const all = computed(() => positions.data.value ?? [])
const protocols = computed(() => protocolsOf(all.value))
const hasClosed = computed(() => all.value.some((position) => position.status === 'closed'))
const visible = computed(() =>
  all.value.filter(
    (position) => matchesProtocol(position, protocol.value) && (showClosed.value || position.status === 'open'),
  ),
)
const opened = computed(() => all.value.find((position) => position.id === openId.value))

/**
 * One place that rewrites the query, so the keys not touched stay as they are. Only opening a position adds a
 * step to the history — a filter or a closed dialog replaces it, so «Back» leaves the screen instead of
 * walking back through every chip the user pressed (the operations screen does the same).
 */
function setQuery(patch: Record<string, string | undefined>, step: 'push' | 'replace' = 'replace') {
  void router[step]({ query: { ...route.query, ...patch } })
}
const pickProtocol = (value: unknown) => {
  if (typeof value === 'string' && value) setQuery({ protocol: value === ALL_PROTOCOLS ? undefined : value })
}

/** A chip's look: the chosen one is filled with the primary colour, as on the other filter rows. */
const CHIP = [
  'h-7 shrink-0 rounded-md border border-border px-2.5 text-caption font-normal text-muted-foreground hover:text-foreground max-md:h-10 max-md:px-3 max-md:text-sm',
  'data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground',
]
</script>

<template>
  <div class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4" data-tour="defi-positions">
    <PageHeader :title="t('cryptoTrack.positions.title')" :description="t('cryptoTrack.positions.pageHint')">
      <template #actions>
        <Button variant="outline" :disabled="positions.isFetching.value" @click="positions.refetch()">
          <RefreshCw class="size-4" :class="positions.isFetching.value && 'animate-spin'" />
          {{ t('actions.refresh') }}
        </Button>
      </template>
    </PageHeader>

    <SectionState
      :loading="positions.isPending.value"
      :error="positions.error.value"
      :empty="!all.length"
      :empty-icon="Layers"
      :empty-title="t('cryptoTrack.positions.empty.title')"
      :empty-description="t('cryptoTrack.positions.empty.description')"
      @retry="positions.refetch()"
    >
      <template #empty>
        <Button as-child>
          <RouterLink :to="{ name: 'stores' }">{{ t('defiOnboarding.connect') }}</RouterLink>
        </Button>
      </template>
      <!--
        The skeleton is as tall as the content (rule 17): on a phone the summary is one card of three rows,
        on a computer three tiles, then the filter row and five rows of 64 px.
      -->
      <template #loading>
        <div class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4">
          <Skeleton v-if="phone" class="h-[7.5rem] w-full" />
          <div v-else class="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <Skeleton v-for="tile in 3" :key="tile" class="h-[5.5rem] w-full" />
          </div>
          <Skeleton class="h-10 w-full md:h-7" />
          <Skeleton v-for="row in 5" :key="row" class="h-16 w-full" />
        </div>
      </template>

      <div class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4">
        <CryptoPositionSummary :positions="visible" />

        <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <ToggleGroup
            :model-value="protocol"
            type="single"
            class="scrollbar-none flex min-w-0 gap-1.5 overflow-x-auto md:flex-wrap"
            :aria-label="t('cryptoTrack.positions.filter')"
            @update:model-value="pickProtocol"
          >
            <ToggleGroupItem :value="ALL_PROTOCOLS" :class="CHIP">{{ t('cryptoTrack.positions.allProtocols') }}</ToggleGroupItem>
            <ToggleGroupItem v-for="family in protocols" :key="family" :value="family" :class="CHIP">
              {{ familyLabel(family) }}
            </ToggleGroupItem>
          </ToggleGroup>
          <Button
            v-if="hasClosed"
            variant="outline"
            size="sm"
            :aria-pressed="showClosed"
            @click="setQuery({ closed: showClosed ? undefined : '1' })"
          >
            {{ showClosed ? t('cryptoTrack.positions.hideClosed') : t('cryptoTrack.positions.showClosed') }}
          </Button>
        </div>

        <CryptoPositionTable
          v-if="visible.length"
          :positions="visible"
          @open="(position) => setQuery({ position: position.id }, 'push')"
        />
        <EmptyState v-else compact :title="t('cryptoTrack.positions.noneMatch')" />

        <p class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.legend') }}</p>
      </div>
    </SectionState>

    <Dialog
      :open="Boolean(opened)"
      :title="t('cryptoTrack.positions.detailTitle')"
      :description="t('cryptoTrack.positions.detailHint')"
      size="lg"
      overlay="crypto-position"
      @update:open="(value) => !value && setQuery({ position: undefined })"
    >
      <CryptoPositionDetail v-if="opened" :position="opened" />
    </Dialog>
  </div>
</template>
