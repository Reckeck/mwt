<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'

import { BudgetDeleteDialog, BudgetDialog } from '@/features/budget-edit'
import { useFocusRow } from '@/shared/lib/use-focus-row'
import { Button, buttonVariants } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import { useBudgetRows } from './model/use-budget-rows'
import BudgetRow from './ui/BudgetRow.vue'

/**
 * The limits as Budget rows. With `editable` the list owns its dialogs (new, edit, delete); without it the
 * same rows are a read-only answer to «how are the categories against their limits» (the month screen).
 */
const props = withDefaults(defineProps<{ editable?: boolean }>(), { editable: false })

const { t } = useI18n()
const { rows, ready, loading, error, empty, refetch } = useBudgetRows()
useFocusRow(ready)

const dialogOpen = ref(false)
const editing = ref<Budget | null>(null)
function openNew() {
  editing.value = null
  dialogOpen.value = true
}
function openEdit(budget: Budget) {
  editing.value = budget
  dialogOpen.value = true
}

const deleting = ref<{ budget: Budget; name: string } | null>(null)
const deleteOpen = ref(false)
function askDelete(budget: Budget, name: string) {
  deleting.value = { budget, name }
  deleteOpen.value = true
}
defineExpose({ openNew })
</script>

<template>
  <SectionState :loading="loading" :error="error" skeleton="list" compact @retry="refetch()">
    <div class="grid gap-1">
      <div v-if="props.editable" class="flex items-center justify-between gap-2">
        <p class="min-w-0 flex-1 text-xs font-medium text-muted-foreground">{{ t('budget.intro') }}</p>
        <Button v-if="!empty" variant="outline" size="sm" data-budget-new @click="openNew"><Plus />{{ t('budget.new') }}</Button>
      </div>
      <EmptyState v-if="empty" compact :title="t('budget.empty')">
        <Button v-if="props.editable" data-budget-new @click="openNew"><Plus class="size-4" />{{ t('budget.new') }}</Button>
        <RouterLink v-else :to="{ name: 'money-plan' }" :class="buttonVariants({ variant: 'outline' })">{{ t('budget.setUp') }}</RouterLink>
      </EmptyState>
      <div v-else class="grid divide-y divide-border">
        <BudgetRow v-for="row in rows" :key="row.budget.id" :budget="row.budget" :status="row.status" :name="row.name">
          <template v-if="props.editable" #actions>
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground max-md:size-10"
              :title="t('budget.edit')"
              :aria-label="t('budget.edit')"
              data-budget-edit
              @click="openEdit(row.budget)"
            >
              <Pencil class="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-destructive max-md:size-10"
              :title="t('budget.delete')"
              :aria-label="t('budget.delete')"
              data-budget-delete-row
              @click="askDelete(row.budget, row.name)"
            >
              <Trash2 class="size-4" />
            </Button>
          </template>
        </BudgetRow>
      </div>
    </div>
  </SectionState>
  <template v-if="props.editable">
    <BudgetDialog v-model:open="dialogOpen" :budget="editing" />
    <BudgetDeleteDialog v-model:open="deleteOpen" :budget="deleting?.budget ?? null" :name="deleting?.name ?? ''" />
  </template>
</template>
