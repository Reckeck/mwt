<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'
import { useSettingsStore } from '@/shared/store/settings'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import StoreLeafList from './StoreLeafList.vue'
import StoreLeafTrigger from './StoreLeafTrigger.vue'
import { leafGroups, pickedLeaf } from './store-leaf-select.lib'

/**
 * One wallet out of the tree (Full-New-Fixes 1): the lists of groups with the
 * sub-accounts under them, each wallet once, a search on top, the balance beside
 * the name. A group is a heading, never an answer; only a sub-account holds money.
 * On a phone the list is a bottom sheet, on a desktop a popover.
 */
const props = defineProps<{ nodes: StoreTreeNode[]; placeholder?: string; invalid?: boolean; id?: string }>()
const storeId = defineModel<string>({ default: '' })

const { t } = useI18n()
const settings = useSettingsStore()
const phone = useIsPhone()
const open = ref(false)
const search = ref('')

const groups = computed(() => leafGroups(props.nodes, search.value))
const picked = computed(() => pickedLeaf(props.nodes, storeId.value))

function pick(id: string) {
  storeId.value = id
  open.value = false
  search.value = ''
}
</script>

<template>
  <!-- A phone gets a bottom sheet (the dialog turns into one), a desktop a popover under the field. -->
  <template v-if="phone">
    <StoreLeafTrigger
      :id="props.id"
      :picked="picked"
      :currency="settings.baseCurrency"
      :placeholder="props.placeholder"
      :invalid="props.invalid"
      :open="open"
      @click="open = true"
    />
    <Dialog v-model:open="open" overlay="store-leaf" :title="props.placeholder ?? t('store.pickLeaf')">
      <div class="grid gap-3">
        <div class="relative">
          <Search class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
          <Input v-model="search" class="h-11 pl-9" :placeholder="t('store.findWallet')" :aria-label="t('store.findWallet')" />
        </div>
        <StoreLeafList :groups="groups" :picked="storeId" :currency="settings.baseCurrency" @pick="pick" />
      </div>
    </Dialog>
  </template>
  <Popover v-else v-model:open="open">
    <PopoverTrigger as-child>
      <StoreLeafTrigger :id="props.id" :picked="picked" :currency="settings.baseCurrency" :placeholder="props.placeholder" :invalid="props.invalid" :open="open" />
    </PopoverTrigger>
    <PopoverContent overlay="store-leaf" align="start" class="w-(--reka-popover-trigger-width) min-w-[min(16rem,calc(100vw-2rem))] p-0">
      <div class="relative border-b border-border">
        <Search class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
        <Input v-model="search" class="h-10 border-0 pl-9 shadow-none focus-visible:ring-0" :placeholder="t('store.findWallet')" :aria-label="t('store.findWallet')" />
      </div>
      <StoreLeafList :groups="groups" :picked="storeId" :currency="settings.baseCurrency" class="max-h-72 overflow-y-auto p-1" @pick="pick" />
    </PopoverContent>
  </Popover>
</template>
