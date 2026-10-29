<script setup lang="ts">
import { useEventListener } from '@vueuse/core'
import { GraduationCap } from 'lucide-vue-next'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { notify } from '@/shared/lib/notify'
import { settingsQuery } from '@/shared/lib/settings-link'
import { useTour } from '@/shared/lib/tour'
import { Button } from '@/shared/ui/button'

/**
 * Full-New-Guide 1-2: a screen's tour is offered by a soft banner on the first
 * visit, never opened by itself. `Show` starts it, `Not now` hides the banner
 * for this screen only, `Don't offer again` switches the offers off for all of them.
 */
const props = defineProps<{ id: string }>()
const emit = defineEmits<{ close: [] }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const tour = useTour()

function show() {
  emit('close')
  tour.start(props.id)
}

function later() {
  emit('close')
  void tour.decline(props.id)
}

async function never() {
  emit('close')
  try {
    await tour.setOffers(false)
    notify.info('tour.offersOff', {
      action: {
        label: t('tour.configure'),
        onClick: () => void router.push({ query: settingsQuery(route.query, 'help') }),
      },
    })
  } catch {
    notify.error(null, 'settings.saveFailed')
  }
}

// Esc hides the banner like `Not now` — unless a window is open above it.
useEventListener(document, 'keydown', (event: KeyboardEvent) => {
  if (event.key === 'Escape' && !document.querySelector('[role="dialog"]')) later()
})
</script>

<template>
  <section
    class="mb-3 flex flex-wrap items-center gap-x-4 gap-y-2 rounded-lg border border-border bg-muted/50 px-3 py-2.5"
    :aria-label="t('tour.offerTitle')"
    data-tour-offer
  >
    <GraduationCap class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />
    <p class="min-w-0 flex-1 basis-48 text-sm">
      <b class="font-semibold">{{ t('tour.offerTitle') }}</b><br />
      <span class="text-muted-foreground">{{ t('tour.offerHint') }}</span>
    </p>
    <div class="flex flex-wrap items-center gap-2">
      <Button size="sm" class="max-md:h-10" @click="show">{{ t('tour.offerShow') }}</Button>
      <Button variant="outline" size="sm" class="max-md:h-10" @click="later">{{ t('tour.offerLater') }}</Button>
      <Button variant="ghost" size="sm" class="text-muted-foreground max-md:h-10" @click="never">{{ t('tour.offerNever') }}</Button>
    </div>
  </section>
</template>
