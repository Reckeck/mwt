<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Phone } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { TapHint } from '@/shared/ui/tap-hint'

import type { PersonNameProps } from './person-name.types'

/** A person of an event as a table cell: the initial, the name, `guest` / `quick`, a phone mark. */
const props = defineProps<PersonNameProps>()
const { t } = useI18n()
const shown = computed(() => props.name || t('sharedEvents.me'))
</script>

<template>
  <div class="flex min-w-0 items-center gap-3">
    <span
      class="flex size-8 shrink-0 items-center justify-center rounded-md bg-secondary text-sm text-muted-foreground"
      aria-hidden="true"
    >
      {{ shown.slice(0, 1).toUpperCase() }}
    </span>
    <span class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
      <span class="truncate font-medium">{{ shown }}</span>
      <Badge v-if="props.kind === 'guest'" variant="outline" class="font-normal">{{ t('sharedEvents.guest') }}</Badge>
      <Badge v-else-if="props.quick" variant="outline" class="font-normal">{{ t('sharedEvents.quick') }}</Badge>
      <TapHint v-if="props.hasDetails" :hint="t('sharedEvents.hasDetails')" :aria-label="t('sharedEvents.hasDetails')">
        <Phone class="size-4 shrink-0 text-muted-foreground" />
      </TapHint>
    </span>
  </div>
</template>
