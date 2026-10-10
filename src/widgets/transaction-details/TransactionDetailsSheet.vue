<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Link2, Link2Off, Redo2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { EnrichmentBanner } from '@/features/enrichment-review'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton } from '@/features/transaction-edit'
import { TransactionLinkDialog } from '@/features/transaction-link'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { call, parseError } from '@/shared/api/ipc'
import { formatDateTime, formatMoney } from '@/shared/lib/format'
import { txId, txKind, txLegs, txMeta, txPrincipalLegs, txSides } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { TransactionDetailsSheetProps } from './transaction-details.types'

const props = defineProps<TransactionDetailsSheetProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()
const context = useTxContext()

const id = computed(() => (props.tx ? txId(props.tx) : ''))
const meta = computed(() => (props.tx ? txMeta(props.tx) : null))

const loss = useQuery({
  queryKey: computed(() => ['tx-loss', id.value]),
  queryFn: () => call<TxLoss>('get_tx_loss', { id: id.value }),
  enabled: computed(() => open.value && Boolean(id.value)),
})
const flows = useQuery({
  queryKey: ['flows'],
  queryFn: () => call<FlowSummary[]>('list_flows', { from: null, to: null }),
  enabled: computed(() => open.value && Boolean(meta.value?.flow_id)),
})
const flow = computed(() => flows.data.value?.find((entry) => entry.flow_id === meta.value?.flow_id) ?? null)

const sides = computed(() => (props.tx ? txSides(props.tx) : { from: null, to: null }))
const rate = computed(() => {
  const legs = props.tx ? txPrincipalLegs(props.tx) : []
  const out = legs.find((leg) => Number(leg.amount) < 0)
  const into = legs.find((leg) => Number(leg.amount) > 0 && leg.asset.symbol !== out?.asset.symbol)
  if (!out || !into) return null
  const value = Math.abs(Number(out.amount)) / Number(into.amount)
  return `1 ${into.asset.symbol} = ${value.toLocaleString(undefined, { maximumFractionDigits: 6 })} ${out.asset.symbol}`
})

/** Where the money of this operation ended up: the next link starts there. */
const continueStore = computed(() => {
  const legs = props.tx ? txLegs(props.tx).filter((leg) => leg.role === 'principal') : []
  return (legs.find((leg) => Number(leg.amount) > 0) ?? legs[0])?.store_id ?? ''
})

const linkOpen = ref(false)
const continueOpen = ref(false)
const continueFlow = ref<string | null>(null)

/** The next link joins this operation's chain; a lone operation starts one first. */
async function continueChain() {
  try {
    continueFlow.value =
      meta.value?.flow_id ?? (await call<string>('link_transactions', { input: { ids: [id.value], flow_id: null } }))
    for (const queryKey of [['transactions'], ['flows']]) void queryClient.invalidateQueries({ queryKey })
    continueOpen.value = true
  } catch (err) {
    toast.error(t('flows.linkFailed'), { description: codeErrorMessage(parseError(err), t) })
  }
}

const unlink = useMutation({
  mutationFn: () => call('unlink_transaction', { id: id.value }),
  onSuccess: () => {
    toast.success(t('flows.unlinked'))
    for (const queryKey of [['transactions'], ['flows'], ['report']]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('flows.linkFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent side="right" class="w-full overflow-y-auto sm:max-w-md">
      <template v-if="props.tx && meta">
        <SheetHeader>
          <SheetTitle>{{ t(`tx.kind.${txKind(props.tx)}`) }}</SheetTitle>
          <SheetDescription>{{ formatDateTime(meta.timestamp) }}</SheetDescription>
        </SheetHeader>

        <div class="grid gap-5 px-4 pb-6 text-sm">
          <section class="grid gap-1">
            <p
              v-for="leg in txPrincipalLegs(props.tx)"
              :key="leg.store_id + leg.asset.symbol"
              class="flex justify-between gap-2 tabular-nums"
            >
              <span class="text-muted-foreground">{{ context.storeName(leg.store_id) }}</span>
              <span class="font-medium">{{ leg.amount }} {{ leg.asset.symbol }}</span>
            </p>
            <p v-if="rate" class="text-xs text-muted-foreground">{{ t('txDetails.rate', { rate }) }}</p>
            <p v-if="sides.from || sides.to" class="text-xs text-muted-foreground">
              {{ sides.from ? context.sideLabel(sides.from) : '—' }} → {{ sides.to ? context.sideLabel(sides.to) : '—' }}
            </p>
          </section>

          <section v-if="loss.data.value" class="grid gap-1">
            <p class="font-medium">{{ t('txDetails.loss') }}</p>
            <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
              <dt class="text-muted-foreground">{{ t('losses.fee') }}</dt>
              <dd class="text-right">{{ formatMoney(loss.data.value.fee, { currency: loss.data.value.currency }) }}</dd>
              <dt class="text-muted-foreground">{{ t('losses.slippage') }}</dt>
              <dd class="text-right">{{ formatMoney(loss.data.value.slippage, { currency: loss.data.value.currency }) }}</dd>
              <dt class="font-medium">{{ t('losses.total') }}</dt>
              <dd class="text-right font-medium">{{ formatMoney(loss.data.value.total, { currency: loss.data.value.currency }) }}</dd>
            </dl>
            <p v-if="loss.data.value.internal" class="text-xs text-muted-foreground">{{ t('losses.internalHint') }}</p>
            <p v-if="!loss.data.value.complete" class="text-xs text-amber-600 dark:text-amber-400">{{ t('losses.incomplete') }}</p>
          </section>

          <section v-if="meta.categories.length || meta.contact_id || meta.comment" class="grid gap-1.5">
            <div v-if="meta.categories.length" class="flex flex-wrap gap-1">
              <Badge v-for="key in meta.categories" :key="key" variant="outline">{{ context.categoryName(key) }}</Badge>
            </div>
            <p v-if="meta.contact_id" class="text-xs">{{ t('tx.field.contact') }}: {{ context.contactName(meta.contact_id) }}</p>
            <p v-if="meta.comment" class="text-xs text-muted-foreground">{{ meta.comment }}</p>
          </section>

          <section class="grid gap-2">
            <p class="font-medium">{{ t('flows.title') }}</p>
            <template v-if="flow">
              <ol class="grid gap-1 text-xs">
                <li v-for="(link, index) in flow.links" :key="txId(link.tx)" class="flex justify-between gap-2" :class="txId(link.tx) === id && 'font-medium'">
                  <span>{{ index + 1 }}. {{ t(`tx.kind.${txKind(link.tx)}`) }} · {{ formatDateTime(txMeta(link.tx).timestamp) }}</span>
                  <span class="tabular-nums text-muted-foreground">{{ formatMoney(link.loss.total, { currency: flow.currency }) }}</span>
                </li>
              </ol>
              <p class="text-xs tabular-nums text-muted-foreground">
                {{ t('flows.result', { gave: formatMoney(flow.gave_value, { currency: flow.currency }), got: formatMoney(flow.got_value, { currency: flow.currency }), loss: formatMoney(flow.loss, { currency: flow.currency }) }) }}
              </p>
            </template>
            <p v-else class="text-xs text-muted-foreground">{{ t('flows.none') }}</p>
            <div class="flex flex-wrap gap-2">
              <Button variant="outline" size="sm" :disabled="!continueStore" @click="continueChain">
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

          <EnrichmentBanner :tx="props.tx" />

          <div class="flex items-center gap-1 border-t border-border pt-3">
            <TransactionEditButton :tx="props.tx" />
            <TransactionDelete :id="id" />
          </div>
        </div>

        <TransactionLinkDialog v-model:open="linkOpen" :tx="props.tx" />
        <TransactionCreateDialog
          v-if="continueStore"
          v-model:open="continueOpen"
          :store-id="continueStore"
          :flow-id="continueFlow"
        />
      </template>
    </SheetContent>
  </Sheet>
</template>
