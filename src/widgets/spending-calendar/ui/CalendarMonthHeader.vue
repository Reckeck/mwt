<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'

/** The month with its arrows, what is left of the month's pool and what each day after today may spend. */
const props = defineProps<{ title: string; currency: string; left: string | null; perDay: string | null }>()
const emit = defineEmits<{ shift: [delta: number] }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1" data-tour="calendar-balance">
    <Button variant="outline" size="icon-sm" class="touch-min" :aria-label="t('calendar.prev')" @click="emit('shift', -1)">
      <ChevronLeft class="size-4" />
    </Button>
    <span class="text-sm font-semibold capitalize">{{ props.title }}</span>
    <Button variant="outline" size="icon-sm" class="touch-min" :aria-label="t('calendar.next')" @click="emit('shift', 1)">
      <ChevronRight class="size-4" />
    </Button>
    <p class="flex min-h-5 w-full min-w-0 flex-wrap md:w-auto items-center gap-x-2 text-sm text-muted-foreground">
      <template v-if="props.left !== null">
        {{ t('calendar.left') }}
        <RouterLink :to="{ name: 'money-month' }" class="font-semibold text-primary">
          <Money :amount="props.left" :decimals="0" :currency="props.currency" :convert="false" />
        </RouterLink>
      </template>
      <template v-if="props.perDay !== null">
        ·
        <span>
          <Money :amount="props.perDay" :decimals="0" :currency="props.currency" :convert="false" />
          {{ t('calendar.perDaySuffix') }}
        </span>
      </template>
    </p>
  </div>
</template>
