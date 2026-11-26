<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useAttachPending } from '@/features/attachment-edit'
import {
  type QuickField,
  type QuickFormState,
  QuickForm,
  emptyQuickForm,
  nextInSeries,
  quickErrors,
  quickFormToInput,
  quickWalletNodes,
  useQuickAsset,
  useQuickSave,
} from '@/features/transaction-create'
import { useStoreTree } from '@/shared/api/queries'
import { toLocalInput } from '@/shared/lib/datetime'
import { useFormValidation } from '@/shared/lib/form'
import { quickAddRequest } from '@/shared/lib/quick-add'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Kbd } from '@/shared/ui/kbd'

import { txId } from '@/shared/lib/transactions'

import { rememberStore, startForm } from '../../lib/quick-add-start'
import { applyMore, emptyQuickMore, moreError } from '../../lib/quick-more'
import { useQuickAddSeries } from '../../model/use-quick-add-series'
import { useQuickOverrun } from '../../model/use-quick-overrun'
import { MissingSummary, OverrunAlert } from '../quick-alerts'
import { QuickMoreSheet } from '../quick-more'

/**
 * `+ Transaction` from anywhere: a bottom sheet on a phone, a dialog on a desktop (`Dialog` is
 * both). Save stays active; a missing field is said under it on save (rule 13). Enter saves and
 * closes, Ctrl Enter saves and starts the next one on the same wallet, category and date.
 */
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const phone = useIsPhone()
const tree = useStoreTree()
const walletIds = computed(() => quickWalletNodes(tree.data.value).map((node) => node.store.id))

const form = ref<QuickFormState>(emptyQuickForm())
const more = ref(emptyQuickMore())
const asset = useQuickAsset(() => form.value.storeId)
const toAsset = useQuickAsset(() => form.value.toStoreId)
const check = useFormValidation<QuickField | 'fee'>(() => ({ ...quickErrors(form.value), fee: moreError(more.value) }), {
  resetOn: () => form.value.kind,
})
/** A save asked before the wallet's asset is known waits for it instead of failing. */
const waitingForAsset = ref(false)
const attach = useAttachPending()
const overrun = useQuickOverrun(() => ({
  kind: form.value.kind,
  category: form.value.category,
  amount: form.value.amount,
  currency: asset.value?.symbol ?? '',
}))

/** The messages under the fields: only after a save was tried (rule 13). */
const shown = computed(() =>
  Object.fromEntries(Object.keys(check.errors.value).map((field) => [field, check.error(field as QuickField)])),
)
/** The same, as one line above the save button: what is missing, by the field's name. */
const FIELD_LABELS: Record<QuickField | 'fee', string> = {
  amount: 'form.sum',
  storeId: 'quickAdd.store',
  toStoreId: 'tx.quick.toWallet',
  toAmount: 'quickAdd.got',
  category: 'quickAdd.category',
  timestamp: 'quickAdd.date',
  fee: 'tx.fee',
}
const missing = computed(() =>
  (Object.keys(check.errors.value) as Array<QuickField | 'fee'>)
    .filter((field) => check.error(field) !== undefined)
    .map((field) => t(FIELD_LABELS[field])),
)

const series = useQuickAddSeries({
  close: () => (open.value = false),
  next: async () => {
    form.value = nextInSeries(form.value)
    more.value = emptyQuickMore()
    check.reset()
    await nextTick()
    document.getElementById('quick-amount')?.focus()
  },
})
const save = useQuickSave((tx) => {
  rememberStore(form.value.storeId)
  const files = more.value.files
  series.saved(tx)
  void attach(txId(tx), files)
})

const title = computed(() =>
  series.series.value ? `${t('quickAdd.title')} · ${t('quickAdd.inRow', { n: series.series.value + 1 })}` : t('quickAdd.title'),
)

// Each opening starts fresh; the wallets arriving later still fill an empty wallet field.
watch(
  open,
  (isOpen) => {
    if (!isOpen) {
      quickAddRequest.value = null
      waitingForAsset.value = false
      series.reset()
      check.reset()
      return
    }
    form.value = startForm(quickAddRequest.value, walletIds.value, toLocalInput(new Date().toISOString()))
    more.value = emptyQuickMore()
  },
  { immediate: true },
)
watch(walletIds, (ids) => {
  if (open.value && !form.value.storeId) form.value.storeId = ids[0] ?? ''
})

const assetsReady = computed(() => Boolean(asset.value) && (form.value.kind !== 'convert' || Boolean(toAsset.value)))

function write() {
  waitingForAsset.value = false
  const input = quickFormToInput(form.value, asset.value as AssetRef, toAsset.value)
  save.mutate(applyMore(input, more.value, asset.value as AssetRef, form.value.kind))
}
watch(assetsReady, (ready) => {
  if (ready && waitingForAsset.value) write()
})

function submit(another: boolean, event?: Event) {
  series.intend(another)
  check.submit(() => {
    if (assetsReady.value) write()
    else waitingForAsset.value = true
  }, event)
}
</script>

<template>
  <Dialog v-model:open="open" overlay="quick-add" :title="title" size="form">
    <form
      id="quick-add-form"
      class="@container m-0 min-w-0"
      novalidate
      @submit.prevent="submit(false, $event)"
      @keydown.ctrl.enter.exact.prevent.stop="submit(true, $event)"
      @keydown.meta.enter.exact.prevent.stop="submit(true, $event)"
    >
      <p v-if="tree.isSuccess.value && !walletIds.length" class="text-sm text-muted-foreground">{{ t('quickAdd.noStores') }}</p>
      <div v-else class="grid gap-4">
        <QuickForm v-model:form="form" :errors="shown" />
        <QuickMoreSheet v-model="more" :kind="form.kind" :fee-error="check.error('fee')" />
        <OverrunAlert v-if="overrun" :overrun="overrun" />
        <MissingSummary :labels="missing" />
      </div>
    </form>

    <template #footer>
      <p v-if="!phone" class="mr-auto flex min-w-0 flex-wrap items-center gap-x-1.5 text-xs text-muted-foreground" data-quick-add-keys>
        <Kbd>Enter</Kbd>{{ t('quickAdd.keySave') }} · <Kbd>Ctrl Enter</Kbd>{{ t('quickAdd.keySaveMore') }}
      </p>
      <Button v-if="!phone" variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="quick-add-form" type="submit" data-quick-add-submit :class="phone && 'h-12 w-full'" :loading="save.isPending.value || waitingForAsset">
        {{ t('quickAdd.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
