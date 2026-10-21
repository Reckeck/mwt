<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Pencil, RefreshCw, Wrench, X } from 'lucide-vue-next'

import { sourceValueText } from '@/entities/data-source'
import { formatAsOf } from '@/shared/lib/format'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EntityLink } from '@/shared/ui/entity-link'

/** One source: its value and when it was read, whether it works, where the value goes, and what to do with it. */
const props = defineProps<{ view: DataSourceView; storeNames: Map<string, string>; busy: boolean }>()
const emit = defineEmits<{ edit: []; refresh: []; confirm: [accept: boolean]; remove: [] }>()
const { t } = useI18n()

const source = computed(() => props.view.source)
const value = computed(() => sourceValueText(source.value.last))
const pendingValue = computed(() => sourceValueText(source.value.pending))
const STATUS_VARIANT = { ok: 'secondary', broken: 'destructive', pending: 'outline' } as const

const actions = computed(() => [
  { id: 'edit', label: t('sources.edit'), icon: Pencil, run: () => emit('edit') },
  { id: 'refresh', label: t('sources.refresh'), icon: RefreshCw, disabled: props.busy, run: () => emit('refresh') },
])
</script>

<template>
  <Card size="sm" :data-source="source.id">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <div class="grid min-w-0 gap-0.5">
          <p class="truncate font-medium">{{ source.name }}</p>
          <p class="truncate text-xs text-muted-foreground">{{ source.url }}</p>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <Badge :variant="STATUS_VARIANT[source.status]">{{ t(`sources.status.${source.status}`) }}</Badge>
          <ActionMenu :actions="actions" :label="t('sources.actions')" />
        </div>
      </div>
    </template>
    <div class="grid gap-3 text-sm">
      <div class="flex flex-wrap items-end justify-between gap-2">
        <p class="text-2xl font-semibold tabular-nums">{{ value ?? '—' }}</p>
        <p class="text-xs text-muted-foreground">
          {{ source.checked_at ? formatAsOf(source.checked_at) : t('sources.neverChecked') }}
        </p>
      </div>
      <div v-if="source.status === 'pending'" class="grid gap-2 rounded-md border border-dashed p-2">
        <p>{{ t('sources.pendingHint', { value: pendingValue ?? '—', previous: value ?? '—' }) }}</p>
        <div class="flex flex-wrap gap-2">
          <Button size="sm" :disabled="props.busy" @click="emit('confirm', true)">
            <Check class="size-4" />{{ t('sources.accept') }}
          </Button>
          <Button size="sm" variant="outline" :disabled="props.busy" @click="emit('confirm', false)">
            <X class="size-4" />{{ t('sources.reject') }}
          </Button>
        </div>
      </div>
      <div v-if="source.status === 'broken'" class="grid gap-2">
        <p class="text-destructive">{{ t('sources.brokenHint', { error: source.error ?? '' }) }}</p>
        <Button size="sm" variant="outline" class="justify-self-start" @click="emit('edit')">
          <Wrench class="size-4" />{{ t('sources.fix') }}
        </Button>
      </div>
      <div class="flex flex-wrap items-center gap-1.5">
        <span class="text-xs text-muted-foreground">{{ t('sources.usedBy') }}</span>
        <span v-if="!props.view.used_by.length" class="text-xs text-muted-foreground">{{ t('sources.unused') }}</span>
        <template v-for="use in props.view.used_by" :key="JSON.stringify(use)">
          <Badge v-if="use.kind === 'inflation'" variant="outline">
            {{ t('sources.useInflation', { currency: use.currency.toUpperCase() }) }}
          </Badge>
          <EntityLink v-else-if="use.kind === 'asset_price'" :target="{ kind: 'asset', id: use.asset_key }" />
          <EntityLink
            v-else-if="use.kind === 'deposit_rate'"
            :target="{ kind: 'store', id: use.store_id }"
            :label="props.storeNames.get(use.store_id) ?? ''"
          />
          <EntityLink v-else :target="{ kind: 'workflow', id: use.workflow_id }" :label="use.name" />
        </template>
      </div>
      <div class="flex justify-end">
        <ConfirmButton :label="t('sources.delete')" :confirm-label="t('sources.deleteConfirm')" @confirm="emit('remove')" />
      </div>
    </div>
  </Card>
</template>
