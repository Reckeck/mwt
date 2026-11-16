<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { decimalAdd } from '@/shared/lib/decimal'
import { formatQuantity } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'

import { feesFigure, priceReason, unpricedSymbols } from '@/entities/crypto-position'
import { PositionFigure } from '../position-figure'

/**
 * The block under the breakdown, by the kind of the position: a pool's fees, a supply's collateral and debt, a
 * stake's balance and what it accrued. Every figure is read from the DTO; nothing is derived from a price here.
 */
const props = defineProps<{ position: CryptoPositionView }>()
const { t } = useI18n()

const supplied = computed(() => props.position.assets.filter((asset) => asset.direction === 'in'))
const owed = computed(() => props.position.assets.filter((asset) => asset.direction === 'out'))
/** A side's total in USD; unknown as soon as one of its assets has no price. */
const sideTotal = (assets: CryptoPositionAsset[]): string | null =>
  assets.some((asset) => asset.value_usd === undefined)
    ? null
    : assets.reduce<string | null>((sum, asset) => (sum === null ? null : decimalAdd(sum, asset.value_usd ?? '')), '0')
const tokensOf = (assets: CryptoPositionAsset[]) =>
  assets
    .filter((asset) => asset.value_usd === undefined)
    .map((asset) => asset.symbol.toUpperCase())
    .join(', ')
const fees = computed(() => feesFigure(props.position))
const stakedQuantity = computed(() => props.position.assets.find((asset) => asset.direction === 'in'))
/** Why the accrued figure is a dash: a token with no price, named, or a reading that did not come through. */
const accruedReason = computed(() => priceReason(props.position))
const accruedTokens = computed(() => unpricedSymbols(props.position).join(', '))
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t(`cryptoTrack.positions.side.${props.position.kind}.title`) }}</h3>
    </template>

    <dl v-if="props.position.kind === 'pool'" class="grid gap-4 sm:grid-cols-3">
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.pool.collected') }}</dt>
        <dd class="figure"><PositionFigure :amount="props.position.fees_collected_usd" reason="notRead" /></dd>
      </div>
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.pool.uncollected') }}</dt>
        <dd class="figure">
          <PositionFigure :amount="props.position.fees_uncollected_usd" :approximate="props.position.approximate" reason="notRead" />
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.pool.total') }}</dt>
        <dd class="figure"><PositionFigure :amount="fees.amount" :approximate="props.position.approximate" :reason="fees.reason" /></dd>
      </div>
    </dl>

    <dl v-else-if="props.position.kind === 'lending'" class="grid gap-4 sm:grid-cols-3">
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.lending.supplied') }}</dt>
        <dd class="figure"><PositionFigure :amount="sideTotal(supplied)" reason="noPrice" :tokens="tokensOf(supplied)" /></dd>
      </div>
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.lending.debt') }}</dt>
        <dd class="figure">
          <PositionFigure :amount="owed.length ? sideTotal(owed) : '0'" reason="noPrice" :tokens="tokensOf(owed)" />
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.lending.accrued') }}</dt>
        <dd class="figure">
          <PositionFigure
            :amount="props.position.accrued_usd"
            :approximate="props.position.approximate"
            :reason="accruedReason"
            :tokens="accruedTokens"
          />
        </dd>
      </div>
    </dl>

    <dl v-else class="grid gap-4 sm:grid-cols-2">
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.staking.balance') }}</dt>
        <dd class="figure">
          <template v-if="stakedQuantity">{{ formatQuantity(stakedQuantity.quantity) }} {{ stakedQuantity.symbol.toUpperCase() }}</template>
          <span v-else class="text-muted-foreground">—</span>
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.side.staking.accrued') }}</dt>
        <dd class="figure">
          <PositionFigure
            :amount="props.position.accrued_usd"
            :approximate="props.position.approximate"
            :reason="accruedReason"
            :tokens="accruedTokens"
          />
        </dd>
      </div>
    </dl>
    <p class="mt-3 text-xs text-muted-foreground">{{ t(`cryptoTrack.positions.side.${props.position.kind}.note`) }}</p>
  </Card>
</template>
