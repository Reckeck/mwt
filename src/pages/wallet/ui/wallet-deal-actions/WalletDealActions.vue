<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'

/**
 * The exchange's own operations in its header (OpForm-2 · 9): Перевести and
 * Обменять, then Спот · Фьючерс · DeFi — these exist only here, not in the
 * general «+ Операция». A phone keeps the three trades.
 */
const props = defineProps<{ disabled?: boolean }>()
const emit = defineEmits<{ deal: [kind: TxKind] }>()
const { t } = useI18n()

const MOVES: { kind: TxKind; label: string }[] = [
  { kind: 'transfer', label: 'deal.transfer' },
  { kind: 'convert', label: 'deal.convert' },
]
const TRADES: { kind: TxKind; label: string }[] = [
  { kind: 'trade_spot', label: 'deal.spot' },
  { kind: 'trade_futures', label: 'deal.futures' },
  { kind: 'defi', label: 'deal.defi' },
]
</script>

<template>
  <div class="flex items-center gap-2" data-tour="wallet-deal">
    <Button
      v-for="entry in MOVES"
      :key="entry.kind"
      variant="outline"
      size="sm"
      class="max-md:hidden"
      :disabled="props.disabled"
      :data-deal="entry.kind"
      @click="emit('deal', entry.kind)"
    >
      {{ t(entry.label) }}
    </Button>
    <span class="h-6 w-px bg-border max-md:hidden" aria-hidden="true" />
    <Button
      v-for="entry in TRADES"
      :key="entry.kind"
      variant="secondary"
      size="sm"
      :disabled="props.disabled"
      :data-deal="entry.kind"
      @click="emit('deal', entry.kind)"
    >
      {{ t(entry.label) }}
    </Button>
  </div>
</template>
