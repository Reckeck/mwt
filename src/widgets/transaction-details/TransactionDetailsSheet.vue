<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Copy, CopyPlus, Link2, Link2Off, Redo2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { EnrichmentBanner } from '@/features/enrichment-review'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton } from '@/features/transaction-edit'
import { TransactionLinkDialog } from '@/features/transaction-link'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { call } from '@/shared/api/ipc'
import { formatDateTime, formatMoney, formatQuantity } from '@/shared/lib/format'
import { txId, txKind, txLegs, txMeta, txPrincipalLegs, txSides, txStoreId } from '@/shared/lib/transactions'
import { copyText } from '@/shared/lib/clipboard'
import { useTxContext } from '@/shared/lib/tx-context'
import { notify } from '@/shared/lib/notify'

import type { TransactionDetailsSheetProps } from './transaction-details.types'
import LegPrices from './ui/LegPrices.vue'

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
  return `1 ${into.asset.symbol} = ${formatQuantity(value, 6)} ${out.asset.symbol}`
})

/** Where the money of this operation ended up: the next link starts there. */
const continueStore = computed(() => {
  const legs = props.tx ? txLegs(props.tx).filter((leg) => leg.role === 'principal') : []
  return (legs.find((leg) => Number(leg.amount) > 0) ?? legs[0])?.store_id ?? ''
})

const linkOpen = ref(false)
const duplicateOpen = ref(false)
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
              <span class="font-medium tabular-nums"><Money :amount="leg.amount" :currency="leg.asset.symbol" signed /></span>
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
              <dd class="text-right"><Money :amount="loss.data.value.fee" :currency="loss.data.value.currency" /></dd>
              <dt class="text-muted-foreground">{{ t('losses.slippage') }}</dt>
              <dd class="text-right"><Money :amount="loss.data.value.slippage" :currency="loss.data.value.currency" /></dd>
              <dt class="font-medium">{{ t('losses.total') }}</dt>
              <dd class="text-right font-medium"><Money :amount="loss.data.value.total" :currency="loss.data.value.currency" /></dd>
            </dl>
            <p v-if="loss.data.value.internal" class="text-xs text-muted-foreground">{{ t('losses.internalHint') }}</p>
            <p v-if="!loss.data.value.complete" class="text-xs text-warning">{{ t('losses.incomplete') }}</p>
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
                  <span class="tabular-nums text-muted-foreground"><Money :amount="link.loss.total" :currency="flow.currency" /></span>
                </li>
              </ol>
              <p class="text-xs tabular-nums text-muted-foreground">
                {{ t('flows.result', { gave: formatMoney(flow.gave_value, { currency: flow.currency }), got: formatMoney(flow.got_value, { currency: flow.currency }), loss: formatMoney(flow.loss, { currency: flow.currency }) }) }}
              </p>
            </template>
            <p v-else class="text-xs text-muted-foreground">{{ t('flows.none') }}</p>
            <div class="flex flex-wrap gap-2">
              <Button
                variant="outline"
                size="sm"
                :disabled="!continueStore"
                :title="continueStore ? undefined : t('flows.continueNoStore')"
                @click="continueChain"
              >
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

          <LegPrices :tx="props.tx" />

          <EnrichmentBanner :tx="props.tx" />

          <div class="flex flex-wrap items-center gap-1 border-t border-border pt-3">
            <TransactionEditButton :tx="props.tx" />
            <Button variant="ghost" size="sm" @click="duplicateOpen = true">
              <CopyPlus class="size-4" />
              {{ t('tx.duplicate') }}
            </Button>
            <Button variant="ghost" size="sm" @click="copyText(id)">
              <Copy class="size-4" />
              {{ t('tx.copyId') }}
            </Button>
            <TransactionDelete :id="id" class="ml-auto" />
          </div>
        </div>

        <TransactionLinkDialog v-model:open="linkOpen" :tx="props.tx" />
        <TransactionCreateDialog
          v-if="continueStore"
          v-model:open="continueOpen"
          :store-id="continueStore"
          :flow-id="continueFlow"
        />
        <TransactionCreateDialog
          v-if="duplicateOpen"
          v-model:open="duplicateOpen"
          :store-id="txStoreId(props.tx)"
          :prefill="props.tx"
        />
      </template>
    </SheetContent>
  </Sheet>
</template>
