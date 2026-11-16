<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CHAIN_LABELS } from '@/shared/lib/chains'
import { Badge } from '@/shared/ui/badge'

/** The chain, the closed mark and the «approximate» mark of a position: one place for the row, the card and the detail. */
const props = defineProps<{ position: CryptoPositionView }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center gap-1">
    <Badge variant="outline" class="font-normal">{{ CHAIN_LABELS[props.position.chain] }}</Badge>
    <Badge v-if="props.position.status === 'closed'" variant="secondary" class="font-normal">
      {{ t('cryptoTrack.positions.status.closed') }}
    </Badge>
    <Badge v-if="props.position.approximate" variant="warning">≈ {{ t('cryptoTrack.positions.approximate') }}</Badge>
  </div>
</template>
