<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronDown, CircleCheck, GraduationCap } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { SettingsSection } from '@/shared/ui/settings-section'
import { SwitchRow } from '@/shared/ui/switch-row'
import { useSettingsStore } from '@/shared/store/settings'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { TOURS, tourIdOf, tourLocation, useTour } from '@/shared/lib/tour'
import { notify } from '@/shared/lib/notify'
import { useStoreTree } from '@/shared/api/queries'

/** The learning centre: progress, the next tour, the tour of the screen under the window, every tour. */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const tour = useTour()
const settings = useSettingsStore()

const tree = useStoreTree()
const firstWallet = computed(() => flattenStoreTree(tree.data.value ?? []).find((node) => !node.store.archived)?.store.id ?? null)

/** The sign-in tour is shown before a vault opens, so the learning centre (inside one) cannot start it. */
const tours = computed(() => Object.keys(TOURS).filter((id) => id !== 'unlock').map((id) => ({ id, done: tour.isDone(id) })))
const doneCount = computed(() => tours.value.filter((entry) => entry.done).length)
const next = computed(() => tours.value.find((entry) => !entry.done)?.id ?? null)
const screenTour = computed(() => {
  const id = tourIdOf(String(route.name ?? ''))
  return tour.hasTour(id) ? id : null
})

/** The tour of a screen: another screen is opened with `?tour=`, the current one is started in place. */
function open(id: string) {
  // The settings window is the screen: the tour starts right here.
  if (id === 'settings') {
    tour.start(id)
    return
  }
  if (id === screenTour.value) {
    const { settings: _settings, section: _section, ...rest } = route.query
    void router.replace({ query: rest })
    // Let the window close and the screen show before pointing at it.
    setTimeout(() => tour.start(id), 500)
    return
  }
  const location = tourLocation(id, firstWallet.value)
  if (location) void router.push(location)
}

/** «Предлагать подсказки в новых разделах»: on until the banner's «Больше не предлагать». */
async function setOffers(on: boolean) {
  try {
    await tour.setOffers(on)
  } catch {
    notify.error(null, 'settings.saveFailed')
  }
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
  <SettingsSection :title="t('help.learning', { done: doneCount, total: tours.length })">
    <SwitchRow
      id="tour-offers"
      :model-value="settings.app?.tour_offers ?? true"
      :label="t('tour.offersSwitch')"
      :hint="t('tour.offersSwitchHint')"
      class="mb-3"
      @update:model-value="setOffers"
    />
    <div data-tour="learning-center" class="grid gap-3">
      <ProgressBar :value="tours.length ? (doneCount / tours.length) * 100 : 0" :label="t('help.learning', { done: doneCount, total: tours.length })" />
      <p class="text-sm text-muted-foreground">
        {{ next ? t('help.nextLesson', { name: t(`tourName.${next}`) }) : t('help.allDone') }}
      </p>
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" :disabled="!next" @click="next && open(next)">{{ t('help.continue') }}</Button>
        <Button variant="outline" :disabled="!screenTour" @click="screenTour && open(screenTour)">
          {{ t('help.thisScreen') }}
        </Button>
        <Button variant="ghost" @click="reset">
          <GraduationCap class="size-4" />
          {{ t('tour.reset') }}
        </Button>
      </div>
      <Collapsible class="grid gap-2" default-open>
        <CollapsibleTrigger as-child>
          <Button variant="ghost" class="w-fit">
            {{ t('help.allTours') }}
            <ChevronDown class="size-4" />
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <ul class="grid gap-1 @xl:grid-cols-2">
            <li
              v-for="entry in tours"
              :key="entry.id"
              class="flex min-h-10 items-center justify-between gap-2 rounded-md border border-border px-3 text-sm"
            >
              <span class="flex min-w-0 items-center gap-2">
                <CircleCheck v-if="entry.done" class="size-4 shrink-0 text-success" />
                <span v-else class="size-4 shrink-0 rounded-full border border-border" />
                <span class="truncate">{{ t(`tourName.${entry.id}`) }}</span>
              </span>
              <Button variant="ghost" size="sm" class="shrink-0 max-md:h-10" @click="open(entry.id)">
                {{ t('tour.show') }}
              </Button>
            </li>
          </ul>
          <!-- The lessons live with the workflows; here only the way there. -->
          <Button variant="link" class="mt-2 h-auto w-fit p-0" @click="router.push({ name: 'automations-workflows' })">
            {{ t('tour.lessonsLink') }}
          </Button>
        </CollapsibleContent>
      </Collapsible>
    </div>
  </SettingsSection>
</template>
