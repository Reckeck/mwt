<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'

import type { WalletAction } from '../../lib/wallet-actions'

/**
 * The wallet's own buttons (Wallets-1 · 5–7): a row beside the sum on a
 * computer, a grid of equal cells under it on a phone. Each opens the form
 * already on this wallet and kind.
 */
const props = defineProps<{ actions: WalletAction[]; phone: boolean; disabled?: boolean }>()
const emit = defineEmits<{ act: [kind: TxKind] }>()
const { t } = useI18n()

const shown = computed(() => (props.phone ? props.actions.filter((action) => action.phone) : props.actions))
</script>

<template>
  <div
    v-if="props.phone && shown.length"
    class="grid gap-2"
    :style="{ gridTemplateColumns: `repeat(${shown.length}, minmax(0, 1fr))` }"
    data-tour="wallet-actions"
  >
    <Button
      v-for="action in shown"
      :key="action.kind"
      :variant="action.variant"
      class="h-11 px-1 text-caption"
      :disabled="props.disabled"
      :data-deal="action.kind"
      @click="emit('act', action.kind)"
    >
      {{ t(action.label) }}
    </Button>
  </div>
  <div v-else-if="shown.length" class="flex items-center gap-2" data-tour="wallet-actions">
    <template v-for="action in shown" :key="action.kind">
      <span v-if="action.separated" class="h-6 w-px bg-border" aria-hidden="true" />
      <Button :variant="action.variant" :disabled="props.disabled" :data-deal="action.kind" @click="emit('act', action.kind)">
        {{ t(action.label) }}
      </Button>
    </template>
  </div>
</template>
