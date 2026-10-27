<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { useStoreTree } from '@/shared/api/queries'
import { openQuickAdd } from '@/shared/lib/quick-add'
import { useIsPhone } from '@/shared/lib/phone'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'

/**
 * A wallet with nothing in it (Wallets-3 · 18): the first step instead of an
 * empty table — the balance it starts with (not income), or a transfer from a
 * card into it.
 */
const props = defineProps<{ store: Store }>()
const emit = defineEmits<{ balance: [] }>()
const { t } = useI18n()
const phone = useIsPhone()
const tree = useStoreTree()

const where = computed(() => (props.store.kind === 'cash' ? 'cash' : 'account'))
/** The transfer starts on a card: the first bank wallet other than this one. */
const card = computed(
  () =>
    flattenStoreTree(tree.data.value ?? []).find(
      (node) => node.store.kind === 'bank' && !node.store.archived && node.store.id !== props.store.id,
    )?.store.id,
)
function withdraw() {
  openQuickAdd({ storeId: card.value, kind: 'transfer', toStoreId: props.store.id })
}
const BUTTON = 'max-md:h-11 max-md:w-full'
</script>

<template>
  <div
    :class="
      cn(
        'flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border text-center text-muted-foreground',
        phone ? 'px-4 py-8' : 'min-h-[max(22rem,calc(100dvh-10.5rem))] p-12',
      )
    "
    data-wallet-empty
  >
    <b v-if="phone" class="text-sm text-foreground">{{ t('wallet.empty.phoneTitle') }}</b>
    <h2 v-else class="text-base font-semibold text-foreground">{{ t('wallet.empty.title', { name: props.store.name }) }}</h2>
    <p :class="phone ? 'text-[0.8125rem]' : 'text-sm'">{{ t(`wallet.empty.${phone ? 'phoneHint' : 'hint'}.${where}`) }}</p>
    <div class="flex gap-2 max-md:w-full max-md:flex-col">
      <Button :class="BUTTON" :disabled="props.store.archived" data-wallet-balance @click="emit('balance')">{{ t('wallet.empty.balance') }}</Button>
      <Button variant="outline" :class="BUTTON" :disabled="props.store.archived" data-wallet-withdraw @click="withdraw">
        {{ t(props.store.kind === 'cash' ? 'wallet.empty.withdraw' : 'wallet.empty.transferIn') }}
      </Button>
    </div>
  </div>
</template>
