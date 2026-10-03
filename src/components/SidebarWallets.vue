<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { formatMoney } from '@/lib/format'
import { call } from '@/lib/ipc'
import { cn } from '@/lib/utils'

defineProps<{ showLabels: boolean }>()

const { t } = useI18n()
const route = useRoute()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const activeId = computed(() => (route.name === 'wallet' ? String(route.params.id ?? '') : ''))

const base = computed(() => summary.data.value?.base_currency ?? '')

/** Live stores first (alphabetical), archived ones tucked at the bottom. */
const wallets = computed(() => {
  const list = stores.data.value ?? []
  return [...list].sort((a, b) => {
    if (a.archived !== b.archived) return a.archived ? 1 : -1
    return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
  })
})

function valueOf(storeId: string): string {
  const row = summary.data.value?.by_store.find((s) => s.store_id === storeId)
  if (!row) return ''
  return formatMoney(row.value_base, { currency: base.value, decimals: 0 })
}
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto pb-2">
    <p
      v-if="showLabels"
      class="px-3 pb-1 pt-3 text-xs font-medium uppercase tracking-wide text-muted-foreground"
    >
      {{ t('nav.wallets') }}
    </p>

    <nav class="space-y-0.5 px-2">
      <RouterLink
        v-for="w in wallets"
        :key="w.id"
        :to="{ name: 'wallet', params: { id: w.id } }"
        :title="showLabels ? undefined : w.name"
        class="flex items-center gap-2.5 rounded-md px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        :class="cn(activeId === w.id && 'bg-accent text-accent-foreground font-medium', !showLabels && 'justify-center px-0')"
      >
        <span
          class="size-2.5 shrink-0 rounded-full ring-2 ring-background"
          :class="w.archived && 'opacity-40'"
          :style="{ backgroundColor: w.color }"
        />
        <span v-if="showLabels" class="min-w-0 flex-1 truncate">{{ w.name }}</span>
        <span
          v-if="showLabels && valueOf(w.id)"
          class="shrink-0 text-xs tabular-nums text-muted-foreground"
        >
          {{ valueOf(w.id) }}
        </span>
      </RouterLink>

      <RouterLink
        v-if="!stores.data.value?.length"
        :to="{ name: 'stores' }"
        class="flex items-center gap-2.5 rounded-md px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        :class="cn(!showLabels && 'justify-center px-0')"
      >
        <Plus class="size-4 shrink-0" />
        <span v-if="showLabels" class="truncate">{{ t('nav.addWallet') }}</span>
      </RouterLink>
    </nav>

    <RouterLink
      v-if="stores.data.value?.length"
      :to="{ name: 'stores' }"
      :title="showLabels ? undefined : t('nav.addWallet')"
      class="mx-2 mt-1 flex items-center gap-2.5 rounded-md border border-dashed border-border px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
      :class="cn(!showLabels && 'justify-center px-0')"
    >
      <Plus class="size-4 shrink-0" />
      <span v-if="showLabels" class="truncate">{{ t('nav.addWallet') }}</span>
    </RouterLink>
  </div>
</template>
