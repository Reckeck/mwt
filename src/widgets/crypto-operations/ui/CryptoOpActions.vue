<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, ExternalLink, Link2Off, Tag, Trash2 } from 'lucide-vue-next'

import { openLink } from '@/entities/crypto-tx'
import { type CryptoActions, useRemoveContractRule, useUnlinkExchange } from '@/features/crypto-tx-override'
import { notify } from '@/shared/lib/notify'
import { Button } from '@/shared/ui/button'

/**
 * The buttons of an operation's card. The meaning and the exchange open their dialogs; the explorer page is
 * opened here and only here, on this click, with the address that came with the card (`https` or nothing).
 */
const props = defineProps<{ detail: CryptoOperationDetail; menu: CryptoActions }>()
const { t } = useI18n()
const unlink = useUnlinkExchange()
const removeRule = useRemoveContractRule()
const linked = computed(() => Boolean(props.detail.row.flow_id))
/** The module's own entries: no chain to declare, no meaning to correct, no page to open. */
const own = computed(() => Boolean(props.detail.row.own_entry))
const fiatMember = computed(() => props.detail.chain.find((member) => member.side === 'fiat'))

function chainAction() {
  if (linked.value) unlink.mutate({ id: props.detail.row.id, transactionId: fiatMember.value?.id })
  else props.menu.askExchange(props.detail.row)
}

async function openExplorer() {
  try {
    if (!(await openLink(props.detail.url))) notify.warning(t('cryptoTrack.operations.noLink'))
  } catch (error) {
    notify.error(error, 'cryptoTrack.operations.openFailed')
  }
}
</script>

<template>
  <!-- One button for the chain: it declares the exchange, or breaks the link once there is one. -->
  <Button v-if="!own" variant="outline" data-crypto-chain-action :disabled="unlink.isPending.value" @click="chainAction">
    <component :is="linked ? Link2Off : ArrowLeftRight" class="size-4" />
    {{ linked ? t('cryptoTrack.operations.detail.unlink') : t('cryptoTrack.operations.menu.exchange') }}
  </Button>
  <Button v-if="!own" variant="outline" data-tour="crypto-tx-override" @click="props.menu.askMeaning(props.detail.row)">
    <Tag class="size-4" />{{ t('cryptoTrack.operations.detail.editMeaning') }}
  </Button>
  <Button
    v-if="props.detail.contract_rule"
    variant="outline"
    data-crypto-rule-remove
    :disabled="removeRule.isPending.value"
    @click="removeRule.mutate({ id: props.detail.row.id })"
  >
    <Trash2 class="size-4" />{{ t('cryptoTrack.operations.detail.removeRule') }}
  </Button>
  <Button v-if="props.detail.url" variant="outline" data-tour="crypto-tx-link" @click="openExplorer">
    <ExternalLink class="size-4" />{{ t('cryptoTrack.operations.menu.explorer') }}
  </Button>
</template>
