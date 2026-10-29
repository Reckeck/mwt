<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { DatabaseZap, Plus } from 'lucide-vue-next'

import { useDataSources } from '@/entities/data-source'
import { SourceDialog, useSourceActions } from '@/features/data-source-edit'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import SourceRow from './ui/SourceRow.vue'

/** `Sources` (plan I2): the user's own pages, answers and files read on a schedule, and where their values go. */
const { t } = useI18n()
const sources = useDataSources()
const tree = useStoreTree()
const actions = useSourceActions()

const storeNames = computed(
  () => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name])),
)
const busy = computed(() => actions.refresh.isPending.value || actions.confirm.isPending.value)

const dialogOpen = ref(false)
const editing = ref<DataSourceView | null>(null)
function openDialog(view: DataSourceView | null) {
  editing.value = view
  dialogOpen.value = true
}
</script>

<template>
  <section class="grid gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="grid gap-0.5">
        <h2 class="font-medium">{{ t('sources.title') }}</h2>
        <p class="text-sm text-muted-foreground">{{ t('sources.hint') }}</p>
      </div>
      <Button size="sm" data-tour="sources-add" @click="openDialog(null)">
        <Plus class="size-4" />{{ t('sources.add') }}
      </Button>
    </div>
    <SectionState
      :loading="sources.isPending.value"
      :error="sources.error.value"
      skeleton="list"
      :rows="2"
      @retry="sources.refetch()"
    >
      <EmptyState
        v-if="!sources.data.value?.length"
        :icon="DatabaseZap"
        :title="t('sources.emptyTitle')"
        :description="t('sources.emptyHint')"
      >
        <Button size="sm" @click="openDialog(null)"><Plus class="size-4" />{{ t('sources.add') }}</Button>
      </EmptyState>
      <div v-else class="grid gap-3 md:grid-cols-2" data-tour="sources-list">
        <SourceRow
          v-for="view in sources.data.value"
          :key="view.source.id"
          :view="view"
          :store-names="storeNames"
          :busy="busy"
          @edit="openDialog(view)"
          @refresh="actions.refresh.mutate(view.source.id)"
          @confirm="(accept) => actions.confirm.mutate({ id: view.source.id, accept })"
          @remove="actions.remove.mutate(view.source.id)"
        />
      </div>
    </SectionState>
    <SourceDialog v-model:open="dialogOpen" :view="editing" />
  </section>
</template>
