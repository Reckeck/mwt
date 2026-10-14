<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CircleCheck, GraduationCap, PlayCircle, Sparkles } from 'lucide-vue-next'

import { LessonList } from '@/entities/workflow'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { TOURS, tourLocation, useTour } from '@/shared/lib/tour'
import { notify } from '@/shared/lib/notify'

/** The learning centre: every screen's tour, what is done, and «Что нового». */
const { t } = useI18n()
const router = useRouter()
const tour = useTour()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const firstWallet = computed(() => flattenStoreTree(tree.data.value ?? []).find((node) => !node.store.archived)?.store.id ?? null)

const tours = computed(() =>
  Object.keys(TOURS).map((id) => ({ id, done: tour.isDone(id), location: tourLocation(id, firstWallet.value) })),
)
const doneCount = computed(() => tours.value.filter((entry) => entry.done).length)

function show(location: ReturnType<typeof tourLocation>) {
  if (location) void router.push(location)
}

async function reset() {
  try {
    await tour.resetAll()
    notify.success('tour.resetDone')
  } catch {
    notify.error(null, 'settings.saveFailed')
  }
}
</script>

<template>
  <Card>
    <div class="grid gap-4" data-tour="learning-center">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="grid gap-0.5">
          <p class="text-sm font-medium">{{ t('tour.centerTitle') }}</p>
          <p class="text-xs text-muted-foreground">
            {{ t('tour.centerHint', { done: doneCount, total: tours.length }) }}
          </p>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" @click="router.push({ query: { whatsNew: '1' } })">
            <Sparkles class="size-4" />
            {{ t('whatsNew.title') }}
          </Button>
          <Button variant="outline" @click="reset">
            <GraduationCap class="size-4" />
            {{ t('tour.reset') }}
          </Button>
        </div>
      </div>
      <ul class="grid gap-1 @sm:grid-cols-2">
        <li
          v-for="entry in tours"
          :key="entry.id"
          class="flex items-center justify-between gap-2 rounded-md border border-border px-3 py-1.5 text-sm"
        >
          <span class="flex min-w-0 items-center gap-2">
            <CircleCheck v-if="entry.done" class="size-4 shrink-0 text-success" />
            <span v-else class="size-4 shrink-0 rounded-full border border-border" />
            <span class="truncate">{{ t(`tourName.${entry.id}`) }}</span>
          </span>
          <Button
            variant="ghost"
            size="icon"
            class="size-7 shrink-0"
            :disabled="!entry.location"
            :title="t('tour.start')"
            :aria-label="t('tour.start')"
            @click="show(entry.location)"
          >
            <PlayCircle class="size-4" />
          </Button>
        </li>
      </ul>
      <LessonList />
    </div>
  </Card>
</template>
