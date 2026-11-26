<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, DatabaseZap, Pencil, Plus, RefreshCw, Trash2, X } from 'lucide-vue-next'

import { SourceDialog, useSourceActions } from '@/features/data-source-edit'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { formatAsOf } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import { sourceValueText } from '../lib/source-value'
import { useSources } from '../model/use-sources'

/** The user's own pages, answers and files read on a schedule: value, status, where it goes. */
const { t } = useI18n()
const { list, refresh, confirm } = useSources()
const actions = useSourceActions()
const dialogOpen = ref(false)
const editing = ref<DataSourceView | null>(null)
function openDialog(view: DataSourceView | null) {
  editing.value = view
  dialogOpen.value = true
}
const busy = computed(() => refresh.isPending.value || confirm.isPending.value)
const STATUS_VARIANT = { ok: 'secondary', broken: 'destructive', pending: 'outline' } as const

function uses(view: DataSourceView): string {
  const names = view.used_by.map((use) => {
    if (use.kind === 'inflation') return t('sources.useInflation', { currency: use.currency.toUpperCase() })
    if (use.kind === 'workflow') return use.name
    return use.kind === 'asset_price' ? use.asset_key : t('sources.uses.deposit')
  })
  return names.length ? names.join(', ') : t('sources.unused')
}
</script>

<template>
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="grid gap-0.5">
        <h2 class="font-medium">{{ t('sources.title') }}</h2>
        <p class="text-sm text-muted-foreground">{{ t('sources.hint') }}</p>
      </div>
      <Button size="sm" data-tour="source-new" @click="openDialog(null)">
        <Plus class="size-4" />
        {{ t('sources.add') }}
      </Button>
    </div>
    <SectionState
      :loading="list.isPending.value"
      :error="list.error.value"
      :empty="!list.data.value?.length"
      :empty-title="t('sources.emptyTitle')"
      :empty-description="t('sources.emptyHint')"
      :empty-icon="DatabaseZap"
      :rows="3"
      @retry="list.refetch()"
    >
      <div class="grid gap-3 @2xl:grid-cols-2">
        <Card v-for="view in list.data.value" :key="view.source.id" size="sm" :data-source="view.source.id">
          <template #header>
            <div class="flex items-center justify-between gap-2">
              <div class="grid min-w-0 gap-0.5">
                <p class="truncate font-medium">{{ view.source.name }}</p>
                <p class="truncate text-xs text-muted-foreground">{{ view.source.url }}</p>
              </div>
              <Badge :variant="STATUS_VARIANT[view.source.status]">{{ t(`sources.status.${view.source.status}`) }}</Badge>
            </div>
          </template>
          <div class="grid gap-2 text-sm">
            <div class="flex flex-wrap items-end justify-between gap-2">
              <p class="text-2xl font-semibold tabular-nums">{{ sourceValueText(view.source.last) ?? '—' }}</p>
              <p class="text-xs text-muted-foreground">
                {{ view.source.checked_at ? formatAsOf(view.source.checked_at) : t('sources.neverChecked') }}
              </p>
            </div>
            <div v-if="view.source.status === 'pending'" class="grid gap-2 rounded-md border border-dashed p-2">
              <p>
                {{ t('sources.pendingHint', { value: sourceValueText(view.source.pending) ?? '—', previous: sourceValueText(view.source.last) ?? '—' }) }}
              </p>
              <div class="flex flex-wrap gap-2">
                <Button size="sm" :disabled="busy" @click="confirm.mutate({ id: view.source.id, accept: true })">
                  <Check class="size-4" />{{ t('sources.accept') }}
                </Button>
                <Button size="sm" variant="outline" :disabled="busy" @click="confirm.mutate({ id: view.source.id, accept: false })">
                  <X class="size-4" />{{ t('sources.reject') }}
                </Button>
              </div>
            </div>
            <p v-else-if="view.source.status === 'broken'" class="text-destructive">
              {{ t('sources.brokenHint', { error: view.source.error ?? '' }) }}
            </p>
            <div class="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
              <span class="min-w-0 truncate">{{ t('sources.usedBy') }} {{ uses(view) }}</span>
              <div class="flex items-center gap-2">
                <Button size="sm" variant="outline" :disabled="busy" @click="refresh.mutate(view.source.id)">
                  <RefreshCw class="size-4" />{{ t('sources.refresh') }}
                </Button>
                <Button size="icon-sm" variant="ghost" :aria-label="view.source.status === 'broken' ? t('sources.fix') : t('sources.edit')" @click="openDialog(view)">
                  <Pencil class="size-4" />
                </Button>
                <ConfirmButton :label="t('sources.delete')" :confirm-label="t('sources.deleteConfirm')" @confirm="actions.remove.mutate(view.source.id)">
                  <Trash2 class="size-4" />
                </ConfirmButton>
              </div>
            </div>
          </div>
        </Card>
      </div>
    </SectionState>
    <SourceDialog v-model:open="dialogOpen" :view="editing" />
  </div>
</template>
