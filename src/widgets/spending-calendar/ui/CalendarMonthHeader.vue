<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'

/**
 * The month and its one mark: what is left of the month's pool and what each
 * day after today may spend. The rest opens Деньги › Остаток месяца.
 */
const props = defineProps<{
  title: string
  currency: string
  left: string | null
  perDay: string | null
  compact: boolean
}>()
const emit = defineEmits<{ shift: [delta: number] }>()
const { t } = useI18n()
</script>

<template>
  <div v-if="!props.compact" class="flex min-w-0 flex-wrap items-center justify-end gap-2" data-tour="calendar-balance">
    <Button variant="outline" size="icon-sm" :aria-label="t('calendar.prev')" @click="emit('shift', -1)">
      <ChevronLeft class="size-4" />
    </Button>
    <span class="text-sm font-semibold capitalize">{{ props.title }}</span>
    <template v-if="props.left !== null">
      <span class="text-sm text-muted-foreground">{{ t('calendar.left') }}</span>
      <RouterLink :to="{ name: 'money-month' }" class="border-b border-dashed border-current font-semibold text-primary">
        <Money :amount="props.left" :decimals="0" :currency="props.currency" :convert="false" />
      </RouterLink>
    </template>
    <span v-if="props.perDay !== null" class="text-sm text-muted-foreground">
      · {{ t('calendar.canToday') }} <Money :amount="props.perDay" :decimals="0" :currency="props.currency" :convert="false" />
      {{ t('calendar.perDaySuffix') }}
    </span>
    <Button variant="outline" size="icon-sm" :aria-label="t('calendar.next')" @click="emit('shift', 1)">
      <ChevronRight class="size-4" />
    </Button>
  </div>
  <div v-else class="grid gap-1" data-tour="calendar-balance">
    <div class="flex items-center gap-1">
      <span class="flex-1 font-semibold capitalize">{{ props.title }}</span>
      <Button variant="ghost" size="icon" :aria-label="t('calendar.prev')" @click="emit('shift', -1)">
        <ChevronLeft class="size-4" />
      </Button>
      <Button variant="ghost" size="icon" :aria-label="t('calendar.next')" @click="emit('shift', 1)">
        <ChevronRight class="size-4" />
      </Button>
    </div>
    <!-- The line keeps its height while the month loads: the grid below must not drop when the numbers land. -->
    <p class="flex min-h-5 flex-wrap items-center gap-x-2 text-sm">
      <template v-if="props.left !== null">
        <span class="text-muted-foreground">{{ t('calendar.left') }}</span>
        <RouterLink :to="{ name: 'money-month' }" class="font-semibold text-primary">
          <Money :amount="props.left" :decimals="0" :currency="props.currency" :convert="false" />
        </RouterLink>
        <span v-if="props.perDay !== null" class="text-muted-foreground">
          · <Money :amount="props.perDay" :decimals="0" :currency="props.currency" :convert="false" /> {{ t('calendar.perDaySuffix') }}
        </span>
      </template>
    </p>
  </div>
</template>
