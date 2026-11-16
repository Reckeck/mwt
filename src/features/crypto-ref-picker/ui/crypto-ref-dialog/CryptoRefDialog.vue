<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Link2 } from 'lucide-vue-next'

import { CryptoTxRow } from '@/entities/crypto-tx'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { SectionState } from '@/shared/ui/section-state'

import { useCryptoLinkCandidates } from '../../model/use-crypto-link'
import type { CryptoRefDialogProps } from './crypto-ref-dialog.types'

/**
 * The operations near the record's date, the ones whose value fits first. The app only suggests: nothing is
 * linked until a row is pressed, and even then only a visible pointer is made.
 */
const props = defineProps<CryptoRefDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ pick: [row: CryptoOpRow] }>()

const { t } = useI18n()
/** An empty time is «now»; kept as a day, so the query key does not change on every render. */
const time = computed(() => props.time || `${new Date().toISOString().slice(0, 10)}T12:00:00Z`)
const candidates = useCryptoLinkCandidates(
  () => ({ time: time.value, amount: props.amount || undefined, currency: props.currency }),
  open,
)
const options = computed(() => candidates.data.value ?? [])

const reasons = (option: CryptoLinkOption) =>
  [
    option.days === 0 ? t('cryptoTrack.links.sameDay') : t('cryptoTrack.links.nearDate', { days: option.days }),
    option.fits ? t('cryptoTrack.links.fits') : '',
  ].filter(Boolean)

function pick(option: CryptoLinkOption) {
  emit('pick', option.row)
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('cryptoTrack.links.dialogTitle')" :description="t('cryptoTrack.links.dialogHint')" size="form">
    <SectionState
      :loading="candidates.isPending.value"
      :error="candidates.error.value"
      :empty="!options.length"
      :empty-title="t('cryptoTrack.links.emptyTitle')"
      :empty-description="t('cryptoTrack.links.emptyHint')"
      :empty-icon="Link2"
      skeleton="list"
      compact
    >
      <ul class="divide-y divide-border" data-crypto-ref-options>
        <li v-for="option in options" :key="option.row.id" class="relative">
          <CryptoTxRow :row="option.row" />
          <div class="flex flex-wrap items-center gap-2 pb-3">
            <Badge v-for="reason in reasons(option)" :key="reason" variant="outline">{{ reason }}</Badge>
            <Button size="sm" variant="outline" class="ml-auto" data-crypto-ref-pick @click="pick(option)">
              {{ t('cryptoTrack.links.choose') }}
            </Button>
          </div>
        </li>
      </ul>
    </SectionState>
  </Dialog>
</template>
