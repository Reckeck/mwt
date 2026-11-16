<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { CryptoTxDetailBody, cryptoOperationDetailKey, opTitle } from '@/entities/crypto-tx'
import type { CryptoActions } from '@/features/crypto-tx-override'
import { call } from '@/shared/api/ipc'
import { useIsPhone } from '@/shared/lib/phone'
import { Drawer, DrawerContent, DrawerDescription, DrawerTitle } from '@/shared/ui/drawer'
import { SectionState } from '@/shared/ui/section-state'
import { Sheet, SheetContent, SheetDescription, SheetTitle } from '@/shared/ui/sheet'

import CryptoOpActions from './CryptoOpActions.vue'

/**
 * One operation's card (`?op=`): a panel at the right on a computer, a bottom drawer on a phone, `Back`
 * and `Esc` close it. The explorer page is fetched from nowhere: its address comes with the card (built by
 * the vault from what the source stored) and is opened only when `Open in explorer` is pressed.
 */
const props = defineProps<{ id: string; menu: CryptoActions }>()
const emit = defineEmits<{ close: [] }>()

const { t } = useI18n()
const phone = useIsPhone()

const detail = useQuery({
  queryKey: computed(() => cryptoOperationDetailKey(props.id, false)),
  queryFn: () => call<CryptoOperationDetail>('crypto_operation_detail', { input: { id: props.id, with_candidates: false } }),
  enabled: computed(() => Boolean(props.id)),
  placeholderData: keepPreviousData,
})
/** The last card stays while the panel slides out, so it does not empty first. */
const shown = ref<CryptoOperationDetail | null>(null)
watch(
  () => detail.data.value,
  (next) => {
    if (next) shown.value = next
  },
  { immediate: true },
)
const open = computed(() => Boolean(props.id))
const title = computed(() => (shown.value ? opTitle(shown.value.row, t) : ''))
function onOpenChange(next: boolean) {
  if (!next) emit('close')
}
</script>

<template>
  <Drawer v-if="phone" :open="open" @update:open="onOpenChange">
    <DrawerContent layer="crypto-op-sheet" class="h-[calc(100dvh-var(--safe-top)-1.5rem)]">
      <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-[calc(1.5rem+var(--safe-bottom))] pt-2 [scrollbar-gutter:stable]">
        <DrawerTitle class="truncate text-lg font-semibold">{{ title }}</DrawerTitle>
        <DrawerDescription class="mb-3 text-sm text-muted-foreground">{{ t('cryptoTrack.operations.detail.subtitle') }}</DrawerDescription>
        <SectionState :loading="detail.isPending.value && !shown" :error="detail.error.value" :empty="false" @retry="detail.refetch()">
          <CryptoTxDetailBody v-if="shown" :detail="shown">
            <template #actions><CryptoOpActions :detail="shown" :menu="props.menu" /></template>
          </CryptoTxDetailBody>
        </SectionState>
      </div>
    </DrawerContent>
  </Drawer>

  <Sheet v-else :open="open" :modal="false" @update:open="onOpenChange">
    <SheetContent side="right" overlay="crypto-op-sheet" no-overlay class="gap-0 overflow-y-auto [scrollbar-gutter:stable] sm:max-w-[32rem]" @interact-outside.prevent>
      <div class="p-5" data-crypto-op-sheet>
        <SheetTitle class="truncate text-xl">{{ title }}</SheetTitle>
        <SheetDescription class="mb-3 text-sm">{{ t('cryptoTrack.operations.detail.subtitle') }}</SheetDescription>
        <SectionState :loading="detail.isPending.value && !shown" :error="detail.error.value" :empty="false" @retry="detail.refetch()">
          <CryptoTxDetailBody v-if="shown" :detail="shown">
            <template #actions><CryptoOpActions :detail="shown" :menu="props.menu" /></template>
          </CryptoTxDetailBody>
        </SectionState>
      </div>
    </SheetContent>
  </Sheet>
</template>
