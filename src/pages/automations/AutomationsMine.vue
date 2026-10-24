<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { useIsPhone } from '@/shared/lib/phone'
import { SectionState } from '@/shared/ui/section-state'

import type { MineStarter } from './lib/mine-view'
import { useMine } from './model/use-mine'
import { MineChips, MineEmpty, MineList, MineTable } from './ui/mine'

/**
 * «Мои» (Auto-1 · 1–2): everything that fires on its own in one list —
 * repeats, schemes, reminders, price alerts — the soonest first. Empty, it
 * offers four ready-made starts.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const phone = useIsPhone()
const { list, items, filter, storeName, setActive } = useMine()

/** The wizard (Auto-1 · 6) reads `?new=` and starts from that template. */
const start = (starter: MineStarter) => void router.push({ query: { ...route.query, new: starter } })
</script>

<template>
  <SectionState :loading="list.isPending.value" :error="list.error.value" skeleton="list" :rows="5">
    <MineEmpty v-if="!list.data.value?.length" @pick="start" />
    <div v-else class="grid gap-4 max-sm:gap-3" data-tour="automations-mine">
      <div class="flex items-center gap-3">
        <MineChips v-model="filter" />
        <span class="ml-auto text-sm text-muted-foreground max-md:hidden">{{ t('automations.mine.hint') }}</span>
      </div>
      <MineList v-if="phone" :items="items" @toggle="setActive" />
      <MineTable v-else :items="items" :store-name="storeName" @toggle="setActive" />
    </div>
  </SectionState>
</template>
