<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-vue-next'

import { CategorySelect } from '@/features/category-select'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { CATEGORY_GROUPS } from '@/shared/lib/categories'
import { required, useFormValidation } from '@/shared/lib/form'
import { Checkbox } from '@/shared/ui/checkbox'

const { t } = useI18n()
const queryClient = useQueryClient()

const rules = useQuery({
  queryKey: ['categorization-rules'],
  queryFn: () => call<CategorizationRule[]>('list_categorization_rules'),
})
const refresh = () => void queryClient.invalidateQueries({ queryKey: ['categorization-rules'] })

const update = useMutation({
  mutationFn: (input: UpdateRuleInput) => call<CategorizationRule>('update_categorization_rule', { input }),
  onSuccess: refresh,
  meta: { success: 'rules.saved', error: 'settings.saveFailed' },
})
const remove = useMutation({
  mutationFn: (id: string) => call('delete_categorization_rule', { id }),
  onSuccess: refresh,
  meta: { success: 'rules.deleted', error: 'settings.saveFailed' },
})
const reorder = useMutation({
  mutationFn: (ids: string[]) => call('reorder_categorization_rules', { ids }),
  onSuccess: refresh,
  meta: { success: 'rules.reordered', error: 'settings.saveFailed' },
})

/** Swap a rule with its neighbour: rules run top to bottom. */
function move(index: number, step: number) {
  const ids = (rules.data.value ?? []).map((rule) => rule.id)
  const target = index + step
  if (target < 0 || target >= ids.length) return
  ;[ids[index], ids[target]] = [ids[target], ids[index]]
  reorder.mutate(ids)
}

const pattern = ref('')
const category = ref('')
const adder = useFormValidation(() => ({ pattern: required(pattern.value), category: required(category.value, 'pick') }))
const create = useMutation({
  mutationFn: () =>
    call<CategorizationRule>('create_categorization_rule', { input: { pattern: pattern.value.trim(), category: category.value } }),
  onSuccess: () => {
    pattern.value = ''
    adder.reset()
    refresh()
  },
  meta: { success: 'rules.created', error: 'settings.saveFailed' },
})
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t('rules.title') }}</h3>
    </template>

    <div class="grid gap-4">
      <p class="text-xs text-muted-foreground">{{ t('rules.hint') }}</p>

      <form class="grid gap-1" @submit.prevent="adder.submit(create.mutate, $event)">
        <div class="flex flex-wrap items-center gap-2">
          <span class="contents" :data-invalid="adder.invalidAttr('pattern')">
            <Input v-model="pattern" class="h-9 w-full sm:w-56" :placeholder="t('rules.pattern')" :aria-label="t('rules.pattern')" />
          </span>
          <div class="w-full sm:w-56" :data-invalid="adder.invalidAttr('category')">
            <CategorySelect
              v-model="category"
              :groups="CATEGORY_GROUPS"
              :aria-label="t('rules.category')"
              :placeholder="t('rules.category')"
            />
          </div>
          <Button type="submit" size="sm" :disabled="create.isPending.value">
            <Plus class="size-4" />
            {{ t('rules.add') }}
          </Button>
        </div>
        <p v-if="adder.firstMessage()" class="text-xs text-destructive" role="alert">{{ adder.firstMessage() }}</p>
      </form>

      <p v-if="!rules.data.value?.length" class="text-sm text-muted-foreground">{{ t('rules.empty') }}</p>
      <div v-for="(rule, index) in rules.data.value ?? []" :key="rule.id" class="flex items-center gap-2" :class="!rule.active && 'opacity-60'">
        <Checkbox
          :model-value="rule.active"
          :aria-label="t('rules.active')"
          @update:model-value="update.mutate({ ...rule, active: !rule.active })"
        />
        <Input
          class="h-8 min-w-0 flex-1"
          :model-value="rule.pattern"
          :aria-label="t('rules.pattern')"
          @change="(event: Event) => update.mutate({ ...rule, pattern: (event.target as HTMLInputElement).value })"
        />
        <div class="w-48">
          <CategorySelect
            :model-value="rule.category"
            :groups="CATEGORY_GROUPS"
            :aria-label="t('rules.category')"
            @update:model-value="(key) => update.mutate({ ...rule, category: String(key) })"
          />
        </div>
        <Button variant="ghost" size="icon-sm"  :title="t('rules.up')" :aria-label="t('rules.up')" @click="move(index, -1)">
          <ArrowUp class="size-4" />
        </Button>
        <Button variant="ghost" size="icon-sm"  :title="t('rules.down')" :aria-label="t('rules.down')" @click="move(index, 1)">
          <ArrowDown class="size-4" />
        </Button>
        <ConfirmButton :label="t('rules.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(rule.id)">
          <Trash2 class="size-4" />
        </ConfirmButton>
      </div>
    </div>
  </Card>
</template>
