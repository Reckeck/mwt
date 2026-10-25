<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Copy, Link2, Link2Off, Redo2 } from 'lucide-vue-next'

import { Money, MoneyPhrase } from '@/entities/currency'
import { EnrichmentBanner } from '@/features/enrichment-review'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionLinkDialog } from '@/features/transaction-link'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { copyText } from '@/shared/lib/clipboard'
import { formatDateTime } from '@/shared/lib/format'
import { composeSums } from '@/shared/lib/money-parts'
import { notify } from '@/shared/lib/notify'
import { txId, txKind, txLegs, txMeta } from '@/shared/lib/transactions'

import LegPrices from './LegPrices.vue'

/**
 * «Подробнее»: what the card held before Paths 1 · 6 and the frame leaves
 * out — the loss, the chain, coin prices, enrichment, related entities.
 */
const props = defineProps<{ tx: Transaction; open: boolean }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const id = computed(() => txId(props.tx))
const meta = computed(() => txMeta(props.tx))

const loss = useQuery({
  // no placeholder: another operation's loss
  queryKey: computed(() => ['tx-loss', id.value]),
  queryFn: () => call<TxLoss>('get_tx_loss', { id: id.value }),
  enabled: computed(() => props.open && Boolean(id.value)),
})
const flows = useQuery({
  queryKey: ['flows'],
  queryFn: () => call<FlowSummary[]>('list_flows', { from: null, to: null }),
  enabled: computed(() => props.open && Boolean(meta.value.flow_id)),
})
const flow = computed(() => flows.data.value?.find((entry) => entry.flow_id === meta.value.flow_id) ?? null)
const flowResult = computed(() => {
  const chain = flow.value
  if (!chain) return []
  const { mark, parts } = composeSums()
  const sum = (amount: string) => mark({ amount, currency: chain.currency })
  return parts(t('flows.result', { gave: sum(chain.gave_value), got: sum(chain.got_value), loss: sum(chain.loss) }))
})

/** Where the money of this operation ended up: the next link starts there. */
const continueStore = computed(() => {
  const legs = txLegs(props.tx).filter((leg) => leg.role === 'principal')
  return (legs.find((leg) => Number(leg.amount) > 0) ?? legs[0])?.store_id ?? ''
})

const linkOpen = ref(false)
const continueOpen = ref(false)
const continueFlow = ref<string | null>(null)

/** The next link joins this operation's chain; a lone operation starts one first. */
async function continueChain() {
  try {
    continueFlow.value =
      meta.value.flow_id ?? (await call<string>('link_transactions', { input: { ids: [id.value], flow_id: null } }))
    for (const queryKey of [['transactions'], ['flows']]) void queryClient.invalidateQueries({ queryKey })
    continueOpen.value = true
  } catch (err) {
    notify.error(err, 'flows.linkFailed')
  }
}

const unlink = useMutation({
  mutationFn: () => call('unlink_transaction', { id: id.value }),
  onSuccess: () => {
    for (const queryKey of [['transactions'], ['flows'], ['report']]) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'flows.linkFailed', success: 'flows.unlinked' },
})
</script>

<template>
  <AdvancedFields :label="t('txCard.more')" class="text-sm" data-tx-more>
    <section v-if="loss.data.value" class="grid gap-1">
      <p class="font-medium">{{ t('txDetails.loss') }}</p>
      <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
        <dt class="text-muted-foreground">{{ t('losses.fee') }}</dt>
        <dd class="text-right"><Money :amount="loss.data.value.fee" :currency="loss.data.value.currency" /></dd>
        <dt class="text-muted-foreground">{{ t('losses.slippage') }}</dt>
        <dd class="text-right"><Money :amount="loss.data.value.slippage" :currency="loss.data.value.currency" /></dd>
        <dt class="font-medium">{{ t('losses.total') }}</dt>
        <dd class="text-right font-medium"><Money :amount="loss.data.value.total" :currency="loss.data.value.currency" /></dd>
      </dl>
      <p v-if="loss.data.value.internal" class="text-xs text-muted-foreground">{{ t('losses.internalHint') }}</p>
      <p v-if="!loss.data.value.complete" class="text-xs text-warning">{{ t('losses.incomplete') }}</p>
    </section>

    <section class="grid gap-2">
      <p class="font-medium">{{ t('flows.title') }}</p>
      <template v-if="flow">
        <ol class="grid gap-1 text-xs">
          <li v-for="(link, index) in flow.links" :key="txId(link.tx)" class="flex justify-between gap-2" :class="txId(link.tx) === id && 'font-medium'">
            <span>{{ index + 1 }}. {{ t(`tx.kind.${txKind(link.tx)}`) }} · {{ formatDateTime(txMeta(link.tx).timestamp) }}</span>
            <span class="tabular-nums text-muted-foreground"><Money :amount="link.loss.total" :currency="flow.currency" /></span>
          </li>
        </ol>
        <p class="text-xs tabular-nums text-muted-foreground">
          <MoneyPhrase :parts="flowResult" />
        </p>
      </template>
      <p v-else class="text-xs text-muted-foreground">{{ t('flows.none') }}</p>
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" size="sm" :disabled="!continueStore" :title="continueStore ? undefined : t('flows.continueNoStore')" @click="continueChain">
          <Redo2 class="size-4" />
          {{ t('flows.continue') }}
        </Button>
        <Button variant="outline" size="sm" @click="linkOpen = true">
          <Link2 class="size-4" />
          {{ t('flows.linkWith') }}
        </Button>
        <Button v-if="meta.flow_id" variant="ghost" size="sm" :disabled="unlink.isPending.value" @click="unlink.mutate()">
          <Link2Off class="size-4" />
          {{ t('flows.unlink') }}
        </Button>
      </div>
    </section>

    <slot />

    <LegPrices :tx="props.tx" />
    <EnrichmentBanner :tx="props.tx" />

    <Button variant="ghost" size="sm" class="justify-self-start" @click="copyText(id)">
      <Copy class="size-4" />
      {{ t('tx.copyId') }}
    </Button>

    <TransactionLinkDialog v-model:open="linkOpen" :tx="props.tx" />
    <TransactionCreateDialog v-if="continueStore" v-model:open="continueOpen" :store-id="continueStore" :flow-id="continueFlow" />
  </AdvancedFields>
</template>
