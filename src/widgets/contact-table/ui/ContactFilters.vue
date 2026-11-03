<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { cn } from '@/shared/lib/utils'

import type { ContactScope } from '../contact-table.types'

/** Search by name, group or requisite, the scope chips with their counts, and the kind of contact. */
const props = defineProps<{ counts: Record<ContactScope, number> }>()
const search = defineModel<string>('search', { required: true })
const scope = defineModel<ContactScope>('scope', { required: true })
const kind = defineModel<ContactKind | ''>('kind', { required: true })

const { t } = useI18n()

const CHIPS: ContactScope[] = ['all', 'debt', 'quick', 'archive']
const KINDS: ContactKind[] = ['person', 'company', 'exchanger']
const kindModel = computed({ get: () => kind.value, set: (next: string) => (kind.value = next as ContactKind | '') })
</script>

<template>
  <div class="flex min-w-0 flex-wrap items-center gap-2" data-tour="contact-filters">
    <div class="relative min-w-0 flex-1 basis-40 md:w-72 md:flex-none">
      <Search class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input v-model="search" class="pl-9" :placeholder="t('contacts.table.search')" :aria-label="t('contacts.table.search')" />
    </div>
    <Select v-model="kindModel" class="w-32 sm:w-40 md:order-last md:ml-auto" :aria-label="t('contacts.kind')">
      <SelectItem value="">{{ t('contacts.table.anyKind') }}</SelectItem>
      <SelectItem v-for="entry in KINDS" :key="entry" :value="entry">{{ t(`contactKind.${entry}`) }}</SelectItem>
    </Select>
    <!-- On a phone the chips are their own row that scrolls sideways inside itself. -->
    <div data-tour="contact-quick-filter" class="flex min-w-0 gap-2 overflow-x-auto scrollbar-none max-md:w-full md:flex-wrap">
      <Button
        v-for="chip in CHIPS"
        :key="chip"
        :variant="scope === chip ? 'default' : 'outline'"
        :class="cn('shrink-0', scope !== chip && 'text-muted-foreground')"
        :aria-pressed="scope === chip"
        @click="scope = chip"
      >
        {{ t(`contacts.table.scope.${chip}`) }}
        <span class="tabular-nums opacity-70">{{ props.counts[chip] }}</span>
      </Button>
    </div>
  </div>
</template>
