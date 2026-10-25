<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Sparkles } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { WHATS_NEW } from '@/shared/config/whats-new'
import { formatDate } from '@/shared/lib/format'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { tourLocation } from '@/shared/lib/tour'
import { useSettingsStore } from '@/shared/store/settings'
import { useStoreTree } from '@/shared/api/queries'

/**
 * «Что нового»: after an update, what changed and where to see it. Opens by
 * itself for entries not seen yet; `?whatsNew=1` opens the whole history.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const settings = useSettingsStore()

const open = ref(false)
const all = ref(false)

/** Not seen yet and newer than this install: a new user is not handed the whole history. */
const unseen = computed(() => {
  const installed = settings.app?.installed_at ?? ''
  return WHATS_NEW.filter((entry) => !settings.app?.seen_whats_new.includes(entry.id) && entry.date > installed)
})
const entries = computed(() => (all.value ? WHATS_NEW : unseen.value))

watch(
  () => settings.app !== null && unseen.value.length > 0,
  (show) => {
    if (show && !open.value) {
      all.value = false
      open.value = true
    }
  },
  { immediate: true },
)
watch(
  () => route.query.whatsNew,
  (flag) => {
    if (flag !== '1') return
    all.value = true
    open.value = true
    const { whatsNew: _whatsNew, ...rest } = route.query
    void router.replace({ query: rest })
  },
  { immediate: true },
)

async function markSeen() {
  const seen = new Set([...(settings.app?.seen_whats_new ?? []), ...WHATS_NEW.map((entry) => entry.id)])
  try {
    await settings.updateApp({ seen_whats_new: [...seen] })
  } catch {
    // Not remembered: it shows again next time.
  }
}
watch(open, (isOpen) => {
  if (!isOpen && unseen.value.length) void markSeen()
})

const tree = useStoreTree()
const firstWallet = computed(() => flattenStoreTree(tree.data.value ?? []).find((node) => !node.store.archived)?.store.id ?? null)

function show(target: string) {
  const location = tourLocation(target, firstWallet.value)
  if (!location) return
  open.value = false
  void router.push(location)
}
</script>

<template>
  <Dialog overlay="whats-new" v-model:open="open" :title="t('whatsNew.title')" :description="t('whatsNew.hint')" size="lg">
    <div class="grid gap-5">
      <section v-for="entry in entries" :key="entry.id" class="grid gap-2">
        <p class="flex items-center gap-2 text-sm font-medium">
          <Sparkles class="size-4 text-warning" />
          {{ t(`whatsNew.${entry.id}.title`) }}
          <span class="text-xs font-normal text-muted-foreground">{{ formatDate(entry.date) }}</span>
        </p>
        <ul class="grid gap-1.5">
          <li
            v-for="point in entry.points"
            :key="point.key"
            class="flex items-start justify-between gap-3 rounded-md bg-muted/40 px-3 py-2 text-sm"
          >
            <span>{{ t(`whatsNew.${entry.id}.${point.key}`) }}</span>
            <Button v-if="point.route" variant="ghost" size="sm" class="shrink-0" @click="show(point.route)">
              {{ t('whatsNew.show') }}
            </Button>
          </li>
        </ul>
      </section>
    </div>
    <template #footer>
      <Button @click="open = false">{{ t('whatsNew.ok') }}</Button>
    </template>
  </Dialog>
</template>
