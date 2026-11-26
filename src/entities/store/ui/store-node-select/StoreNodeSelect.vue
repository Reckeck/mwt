<script setup lang="ts">
import { computed, ref, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { useSettingsStore } from '@/shared/store/settings'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from '@/shared/ui/sheet'

import { walletRow, type WalletRow } from '../../lib/wallet-option'

/**
 * A wallet picked from a bottom sheet: a search field, then the wallets grouped by what the money
 * is for. The list scrolls inside the sheet, so the search field and the sheet's edge stay
 * reachable with the keyboard up. `id`, `aria-*` and `class` go to the field.
 */
defineOptions({ inheritAttrs: false })
const props = defineProps<{ nodes: StoreTreeNode[]; invalid?: boolean; placeholder?: string }>()
const model = defineModel<string>({ default: '' })
const attrs = useAttrs()
const { t } = useI18n()
const settings = useSettingsStore()

const PURPOSES: Purpose[] = ['everyday', 'savings', 'investment', 'transit']
const open = ref(false)
const search = ref('')

const entries = computed(() =>
  props.nodes.map((node) => ({ purpose: node.store.purpose, row: walletRow(node, t, settings.baseCurrency) })),
)
const current = computed(() => entries.value.find((entry) => entry.row.id === model.value)?.row)
const groups = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return PURPOSES.map((purpose) => ({
    purpose,
    rows: entries.value
      .filter((entry) => entry.purpose === purpose && (!needle || entry.row.haystack.includes(needle) || entry.row.title.toLowerCase().includes(needle)))
      .map((entry): WalletRow => entry.row),
  })).filter((group) => group.rows.length > 0)
})

function pick(id: string) {
  model.value = id
  open.value = false
  search.value = ''
}
</script>

<template>
  <Sheet v-model:open="open">
    <SheetTrigger as-child>
      <Button
        type="button"
        variant="outline"
        v-bind="attrs"
        :aria-invalid="props.invalid || undefined"
        :class="cn('w-full justify-between gap-2 px-3 font-normal', !current && 'text-muted-foreground', props.invalid && 'border-destructive', attrs.class as string)"
      >
        <span class="truncate">{{ current?.title ?? props.placeholder ?? t('quickAdd.pickStore') }}</span>
        <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
      </Button>
    </SheetTrigger>
    <SheetContent side="bottom" overlay="store-select" class="max-h-[85dvh] gap-3 px-4 pt-4" :aria-describedby="undefined">
      <SheetHeader class="p-0">
        <SheetTitle>{{ props.placeholder ?? t('quickAdd.store') }}</SheetTitle>
      </SheetHeader>
      <Input v-model="search" type="search" :aria-label="t('quickAdd.searchStore')" :placeholder="t('quickAdd.searchStore')" autocomplete="off" />
      <div class="min-h-0 flex-1 overflow-y-auto pb-2">
        <p v-if="groups.length === 0" class="py-6 text-center text-sm text-muted-foreground">{{ t('quickAdd.noStore') }}</p>
        <section v-for="group in groups" :key="group.purpose" class="grid gap-1 pb-2">
          <h3 class="px-1 pt-2 text-xs font-medium text-muted-foreground">{{ t(`purpose.${group.purpose}`) }}</h3>
          <Button
            v-for="row in group.rows"
            :key="row.id"
            type="button"
            variant="ghost"
            class="h-auto justify-start gap-3 px-2 py-2 text-left"
            :aria-pressed="row.id === model"
            :class="row.id === model && 'bg-accent'"
            @click="pick(row.id)"
          >
            <component :is="row.icon" class="size-5 shrink-0" :style="row.color ? { color: row.color } : undefined" />
            <span class="grid min-w-0">
              <span class="truncate text-sm">{{ row.title }}</span>
              <span class="truncate text-xs font-normal text-muted-foreground">{{ row.detail }}</span>
            </span>
          </Button>
        </section>
      </div>
    </SheetContent>
  </Sheet>
</template>
