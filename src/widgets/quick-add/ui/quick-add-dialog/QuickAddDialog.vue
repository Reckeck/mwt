<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight } from 'lucide-vue-next'

import { DEAL_GROUPS, DEAL_PHONE_GROUPS, TransactionForm, type TransactionInitial } from '@/features/transaction-create'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Kbd } from '@/shared/ui/kbd'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useIsPhone } from '@/shared/lib/phone'
import { useStoreTree } from '@/shared/api/queries'
import { quickAddRequest } from '@/shared/lib/quick-add'

import { useQuickAddSeries } from '../../model/use-quick-add-series'

/**
 * Ctrl N from anywhere (OpForm-1): the short form, with the wallet as one of
 * its fields — next to the category — and starting on the one used last.
 * Enter saves and closes; Ctrl Enter saves and starts the next one.
 * With `deal` it is «Сделка · <биржа>» (OpForm-2 · 9–11): the exchange's own
 * wallet, fixed, and its tabs Обмен · Перевод | Спот · Фьючерс · DeFi.
 */
const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ deal?: { store: Store; kind: TxKind } | null }>()

const { t } = useI18n()
const phone = useIsPhone()
const LAST_STORE_KEY = 'quick-add.store'

const tree = useStoreTree()
/** An operation is written on a sub-account, never on a group («Основной банк» holds «Карта» and «Накопления»). */
const nodes = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter(
    (node) => !node.store.archived && (props.deal ? true : node.children.filter((child) => !child.store.archived).length === 0),
  ),
)

function remembered(): string {
  try {
    return localStorage.getItem(LAST_STORE_KEY) ?? ''
  } catch {
    return ''
  }
}

const series = useQuickAddSeries(() => (open.value = false))
const title = computed(() => {
  const base = props.deal ? t('deal.title', { name: props.deal.store.name }) : t('quickAdd.title')
  return series.series.value ? `${base} · ${t('quickAdd.inRow', { n: series.series.value + 1 })}` : base
})
const kindGroups = computed(() => (props.deal ? (phone.value ? DEAL_PHONE_GROUPS : DEAL_GROUPS) : null))

const storeId = ref('')
/** Typed before the wallet changed: the form is rebuilt for the new wallet with these in. */
const carried = ref<TransactionInitial | null>(null)
// Each opening starts on the wallet used last time, if it still exists.
watch(
  [open, nodes],
  ([isOpen, list]) => {
    if (!isOpen) {
      quickAddRequest.value = null
      carried.value = null
      series.reset()
      return
    }
    if (props.deal) {
      storeId.value = props.deal.store.id
      return
    }
    const asked = quickAddRequest.value?.storeId
    if (asked && list.some((node) => node.store.id === asked)) {
      storeId.value = asked
      return
    }
    if (storeId.value && list.some((node) => node.store.id === storeId.value)) return
    const last = remembered()
    storeId.value = list.find((node) => node.store.id === last)?.store.id ?? list[0]?.store.id ?? ''
  },
  { immediate: true },
)

const form = ref<InstanceType<typeof TransactionForm> | null>(null)

watch(
  storeId,
  () => {
    if (form.value) carried.value = { ...form.value.draft }
  },
  { flush: 'sync' },
)

/** «Перевести» from a glance, «Записать оплату в „Продукты“» from Ctrl K: the form starts on that kind and category. */
const initial = computed<TransactionInitial | null>(() => {
  if (carried.value) return carried.value
  if (props.deal) return { kind: props.deal.kind, categories: [] }
  const asked = quickAddRequest.value
  if (!asked?.kind) return null
  return {
    kind: asked.kind,
    categories: asked.category ? [asked.category] : [],
    ...(asked.toStoreId ? { to: asked.toStoreId } : {}),
    ...(asked.amount ? { amount: asked.amount } : {}),
  }
})

function onSaved(tx: Transaction) {
  if (!props.deal)
    try {
      localStorage.setItem(LAST_STORE_KEY, storeId.value)
    } catch {
      // Private mode: the choice is simply not remembered.
    }
  series.saved(tx)
}
</script>

<template>
  <Dialog overlay="quick-add" v-model:open="open" :title="title" size="form">
    <fieldset
      class="@container m-0 grid min-w-0 gap-4 border-0 p-0"
      @keydown.enter.exact="series.saveOnce"
      @keydown.ctrl.enter.exact.prevent.stop="series.saveMore"
    >
      <p v-if="tree.isSuccess.value && !nodes.length" class="text-sm text-muted-foreground">{{ t('quickAdd.noStores') }}</p>
      <!-- Keyed by the wallet: the form fills its fields from it once. -->
      <TransactionForm
        v-else-if="open && storeId"
        :key="storeId"
        ref="form"
        compact
        :store-id="storeId"
        :initial="initial"
        :kind-groups="kindGroups"
        @saved="onSaved"
      >
        <template v-if="props.deal" #hint>
          <p class="flex items-center gap-2 text-sm text-muted-foreground" data-deal-hint>
            <ArrowLeftRight class="size-4 shrink-0" />{{ t('deal.hint', { name: props.deal.store.name }) }}
          </p>
        </template>
        <template v-if="!props.deal" #store>
          <FormField for="quick-add-store" :label="t('quickAdd.store')" required>
            <StoreNodeSelect id="quick-add-store" v-model="storeId" :nodes="nodes" class="w-full" />
          </FormField>
        </template>
      </TransactionForm>
    </fieldset>

    <template #footer>
      <template v-if="phone">
        <Button form="transaction-form" type="submit" data-quick-add-submit @click="series.saveOnce" class="h-12 w-full" :loading="form?.pending" :disabled="form?.pending">
          {{ t('quickAdd.submit') }}
        </Button>
      </template>
      <template v-else>
        <p class="mr-auto flex min-w-0 flex-wrap items-center gap-x-1.5 text-xs text-muted-foreground" data-quick-add-keys>
          <Kbd>Enter</Kbd>{{ t('quickAdd.keySave') }} · <Kbd>Ctrl Enter</Kbd>{{ t('quickAdd.keySaveMore') }}
        </p>
        <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
        <Button form="transaction-form" type="submit" data-quick-add-submit @click="series.saveOnce" :loading="form?.pending" :disabled="form?.pending">
          {{ t('quickAdd.submit') }}
        </Button>
      </template>
    </template>
  </Dialog>
</template>
