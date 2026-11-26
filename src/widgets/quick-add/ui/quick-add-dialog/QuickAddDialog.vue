<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

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
import { notify } from '@/shared/lib/notify'
import { quickAddRequest } from '@/shared/lib/quick-add'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Kbd } from '@/shared/ui/kbd'

import { rememberStore, startForm } from '../../lib/quick-add-start'
import { useQuickAddSeries } from '../../model/use-quick-add-series'

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
const asset = useQuickAsset(() => form.value.storeId)
const check = useFormValidation(() => quickErrors(form.value), { resetOn: () => form.value.kind })

/** The messages under the fields: only after a save was tried (rule 13). */
const shown = computed(() =>
  Object.fromEntries(Object.keys(check.errors.value).map((field) => [field, check.error(field as QuickField)])),
)

const series = useQuickAddSeries({
  close: () => (open.value = false),
  next: async () => {
    form.value = nextInSeries(form.value)
    check.reset()
    await nextTick()
    document.getElementById('quick-amount')?.focus()
  },
})
const save = useQuickSave((tx) => {
  rememberStore(form.value.storeId)
  series.saved(tx)
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
      series.reset()
      check.reset()
      return
    }
    form.value = startForm(quickAddRequest.value, walletIds.value, toLocalInput(new Date().toISOString()))
  },
  { immediate: true },
)
watch(walletIds, (ids) => {
  if (open.value && !form.value.storeId) form.value.storeId = ids[0] ?? ''
})

function submit(another: boolean, event?: Event) {
  series.intend(another)
  check.submit(() => {
    if (!asset.value) {
      notify.error(null, 'form.assetRequired')
      return
    }
    save.mutate(quickFormToInput(form.value, asset.value))
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
      <QuickForm v-else v-model:form="form" :errors="shown" />
    </form>

    <template #footer>
      <p v-if="!phone" class="mr-auto flex min-w-0 flex-wrap items-center gap-x-1.5 text-xs text-muted-foreground" data-quick-add-keys>
        <Kbd>Enter</Kbd>{{ t('quickAdd.keySave') }} · <Kbd>Ctrl Enter</Kbd>{{ t('quickAdd.keySaveMore') }}
      </p>
      <Button v-if="!phone" variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="quick-add-form" type="submit" data-quick-add-submit :class="phone && 'h-12 w-full'" :loading="save.isPending.value">
        {{ t('quickAdd.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
