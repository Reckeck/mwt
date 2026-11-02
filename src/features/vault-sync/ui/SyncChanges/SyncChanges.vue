<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown, PenLine } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'

import { collectionKey, sortedChanges } from '../../lib/collections'
import { SyncChangeItem } from '../SyncChangeItem'
import type { SyncChangesProps } from './sync-changes.types'

/** What a sync changes (or changed): counts per kind of record, the records under each, the overwritten fields. */
const props = defineProps<SyncChangesProps>()
const { t } = useI18n()

const groups = computed(() => sortedChanges(props.summary))
</script>

<template>
  <div class="grid gap-2">
    <p v-if="!props.summary.total" class="text-sm text-muted-foreground">{{ t('sync.plan.nothing') }}</p>
    <Collapsible v-for="group in groups" :key="group.collection" class="rounded-md border border-border">
      <CollapsibleTrigger class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
        <span class="min-w-0 flex-1 truncate font-medium">{{ t(collectionKey(group.collection)) }}</span>
        <Badge v-if="group.added" variant="secondary" class="gap-1 tabular-nums">+{{ group.added }}</Badge>
        <Badge v-if="group.updated" variant="outline" class="gap-1 tabular-nums">
          <PenLine class="size-3" aria-hidden="true" />{{ group.updated }}
        </Badge>
        <Badge v-if="group.deleted" variant="destructive" class="gap-1 tabular-nums">−{{ group.deleted }}</Badge>
        <ChevronDown class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <ul class="grid gap-1 border-t border-border px-3 py-2 text-sm">
          <SyncChangeItem v-for="(item, index) in group.items" :key="`${item.key}-${index}`" :item="item" :collection="group.collection" />
          <li v-if="group.added + group.updated + group.deleted > group.items.length" class="text-xs text-muted-foreground">
            {{ t('sync.plan.more', { count: group.added + group.updated + group.deleted - group.items.length }) }}
          </li>
        </ul>
      </CollapsibleContent>
    </Collapsible>

    <div v-if="props.summary.overwritten.length" class="grid gap-1 rounded-md border border-border px-3 py-2 text-sm">
      <p class="font-medium">{{ t('sync.plan.overwritten') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('sync.plan.overwrittenHint') }}</p>
      <ul class="grid gap-1">
        <li v-for="(field, index) in props.summary.overwritten" :key="index" class="min-w-0 truncate">
          <span class="text-muted-foreground">{{ field.label || t(collectionKey(field.collection)) }}:</span>
          {{ ' ' }}<span class="line-through">{{ field.before || '—' }}</span>
          →
          <span>{{ field.after || '—' }}</span>
        </li>
      </ul>
    </div>
  </div>
</template>
