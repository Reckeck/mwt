<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { CryptoPairNote, useCryptoPairs } from '@/entities/crypto-tx'
import { useCryptoAccounts } from '@/entities/crypto-account'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'

import { useLinkCryptoOperation, useUnlinkCryptoOperation } from '../../model/use-crypto-link'
import { CryptoRefDialog } from '../crypto-ref-dialog'
import type { CryptoRefRowProps } from './crypto-ref-row.types'

/**
 * The one row of the fiat form that connects the record to a crypto operation (spec E13). Hidden while the
 * module has no account (and so below security level 3). On a saved record a choice is written at once; on a
 * new one it waits in `pending` and the form writes it after the first save, with the saved id.
 */
const props = defineProps<CryptoRefRowProps>()
const pending = defineModel<CryptoOpRow | null>('pending', { default: null })

const { t } = useI18n()
const accounts = useCryptoAccounts()
const available = computed(() => (accounts.data.value ?? []).length > 0)
const pairs = useCryptoPairs(
  () => (props.txId ? { fiat_tx_id: props.txId } : null),
  { enabled: available },
)
const stored = computed(() => pairs.data.value ?? [])
const link = useLinkCryptoOperation()
const unlink = useUnlinkCryptoOperation()
const picking = ref(false)

function pick(row: CryptoOpRow) {
  if (!props.txId) {
    pending.value = row
    return
  }
  link.mutate({ event_id: row.id, fiat_tx_id: props.txId, previous: null })
}

function remove(pair: CryptoPair) {
  unlink.mutate({ event_id: pair.event_id, fiat_tx_id: pair.fiat_tx_id })
}
</script>

<template>
  <FormField v-if="available" for="tx-crypto-ref" :label="t('cryptoTrack.links.field')" :hint="t('cryptoTrack.links.fieldHint')" data-crypto-ref-row>
    <div id="tx-crypto-ref" class="grid gap-2">
      <div v-for="pair in stored" :key="pair.event_id" class="grid gap-1" data-crypto-ref-pair>
        <CryptoPairNote :pair="pair" show="event" />
        <Button type="button" variant="ghost" size="sm" class="justify-self-start" @click="remove(pair)">
          {{ t('cryptoTrack.links.remove') }}
        </Button>
      </div>
      <div v-if="pending" class="grid gap-1" data-crypto-ref-pending>
        <p class="rounded-lg bg-muted p-3 text-sm">{{ t('cryptoTrack.links.pending') }}</p>
        <Button type="button" variant="ghost" size="sm" class="justify-self-start" @click="pending = null">
          {{ t('cryptoTrack.links.remove') }}
        </Button>
      </div>
      <div>
        <Button type="button" variant="outline" size="sm" @click="picking = true">
          {{ stored.length || pending ? t('cryptoTrack.links.another') : t('cryptoTrack.links.pick') }}
        </Button>
      </div>
    </div>
    <CryptoRefDialog v-model:open="picking" :time="props.time" :amount="props.amount" :currency="props.currency" @pick="pick" />
  </FormField>
</template>
