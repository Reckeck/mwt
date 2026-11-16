<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { type QuarantineRow, shortContract } from '@/entities/crypto-token'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { formatDate, formatQuantity } from '@/shared/lib/format'

import { useTokenReview } from '../../model/use-token-review'

/**
 * Recognise a token (mockup 04): what changes once it counts, and the contract to check in an
 * explorer, because a name can repeat a known one while the contract is a stranger's.
 */
const props = defineProps<{ row: QuarantineRow | null }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const { confirm } = useTokenReview()

function recognize() {
  if (!props.row) return
  confirm.mutate(props.row, { onSuccess: () => (open.value = false) })
}
</script>

<template>
  <Dialog
    v-model:open="open"
    overlay="crypto-token-review"
    size="confirm"
    :title="t('cryptoTrack.review.title', { symbol: props.row?.symbol ?? '' })"
    :description="t('cryptoTrack.review.description')"
  >
    <div v-if="props.row" class="flex flex-col gap-3 text-sm">
      <h3 class="font-semibold">{{ t('cryptoTrack.review.changes') }}</h3>
      <ul class="flex flex-col gap-2">
        <li>{{ t('cryptoTrack.review.counts') }}</li>
        <li>{{ t('cryptoTrack.review.operations', { date: formatDate(props.row.firstSeen) }) }}</li>
        <li>{{ t('cryptoTrack.review.price') }}</li>
      </ul>
      <div class="rounded-lg bg-muted p-3">
        <p class="font-semibold">{{ props.row.symbol }} · {{ formatQuantity(props.row.quantity, 2) }}</p>
        <p class="mt-1 break-all text-muted-foreground">
          {{ t('cryptoTrack.review.contract', { contract: props.row.contract, short: shortContract(props.row.contract) }) }}
          {{ t('cryptoTrack.review.undo') }}
        </p>
      </div>
    </div>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button :loading="confirm.isPending.value" :disabled="confirm.isPending.value" @click="recognize">
        {{ t('cryptoTrack.quarantine.recognize') }}
      </Button>
    </template>
  </Dialog>
</template>
