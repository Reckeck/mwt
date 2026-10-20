<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Plus, Trash2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { CATEGORY_GROUPS, categoryLabel } from '@/shared/lib/categories'
import { required, useFormValidation } from '@/shared/lib/form'
import { useCategories } from '@/shared/api/queries'

const { t } = useI18n()
const queryClient = useQueryClient()

const categories = useCategories()

const sections = computed(() =>
  CATEGORY_GROUPS.map((group) => ({
    group,
    items: (categories.data.value ?? []).filter((category) => category.group === group),
  })),
)

const refresh = () => void queryClient.invalidateQueries({ queryKey: ['categories'] })

const update = useMutation({
  mutationFn: (input: UpdateCategoryInput) => call<Category>('update_category', { input }),
  onSuccess: () => {
    refresh()
  },
  meta: { success: 'categories.manage.renamed', error: 'settings.saveFailed' },
})

const archive = useMutation({
  mutationFn: (category: Category) => call('archive_category', { key: category.key, archived: !category.archived }),
  onSuccess: refresh,
  meta: {
    error: 'settings.saveFailed',
    success: (_data, category) =>
      (category as Category).archived ? t('categories.manage.restored') : t('categories.manage.archived'),
  },
})

const remove = useMutation({
  mutationFn: (key: string) => call('delete_category', { key }),
  onSuccess: () => {
    refresh()
  },
  meta: { success: 'categories.manage.deleted', error: 'categories.manage.deleteFailed' },
})

/** Rename on blur; an empty name returns to the built-in label. */
function rename(category: Category, raw: string) {
  const name = raw.trim()
  if (name === categoryLabel(category, t)) return
  update.mutate({ key: category.key, name: name || null, group: category.group })
}

const newName = ref('')
const newGroup = ref<CategoryGroup>('daily')
const adder = useFormValidation(() => ({ name: required(newName.value) }))
const create = useMutation({
  mutationFn: () => call<Category>('create_category', { input: { key: null, name: newName.value.trim(), group: newGroup.value } }),
  onSuccess: () => {
    newName.value = ''
    adder.reset()
    refresh()
  },
  meta: { success: 'categories.manage.created', error: 'settings.saveFailed' },
})
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t('categories.manage.title') }}</h3>
    </template>

    <div class="grid gap-5">
      <p class="text-xs text-muted-foreground">{{ t('categories.manage.hint') }}</p>

      <form class="grid gap-1" @submit.prevent="adder.submit(create.mutate, $event)">
        <div class="flex flex-wrap items-center gap-2">
          <span class="contents" :data-invalid="adder.invalidAttr('name')">
            <Input
              v-model="newName"
              class="h-9 w-full sm:w-56"
              :placeholder="t('categories.manage.newName')"
              :aria-label="t('categories.manage.newName')"
            />
          </span>
          <Select v-model="newGroup" class="h-9 w-44" :aria-label="t('categories.manage.group')">
            <SelectItem v-for="group in CATEGORY_GROUPS" :key="group" :value="group">{{ t(`categoryGroup.${group}`) }}</SelectItem>
          </Select>
          <Button type="submit" size="sm" :disabled="create.isPending.value">
            <Plus class="size-4" />
            {{ t('categories.manage.add') }}
          </Button>
        </div>
        <p v-if="adder.firstMessage()" class="text-xs text-destructive" role="alert">{{ adder.firstMessage() }}</p>
      </form>

      <div v-for="section in sections" :key="section.group" class="grid gap-1 pointer-coarse:gap-2">
        <p class="text-xs font-medium text-muted-foreground">{{ t(`categoryGroup.${section.group}`) }}</p>
        <div
          v-for="category in section.items"
          :key="category.key"
          class="flex items-center gap-2"
          :class="category.archived && 'opacity-60'"
        >
          <Input
            class="h-8 min-w-0 flex-1"
            :model-value="categoryLabel(category, t)"
            :aria-label="t('categories.manage.name')"
            @change="(event: Event) => rename(category, (event.target as HTMLInputElement).value)"
          />
          <Select
            class="h-8 w-40"
            :model-value="category.group"
            :aria-label="t('categories.manage.group')"
            @update:model-value="(group) => update.mutate({ key: category.key, name: category.name, group: group as CategoryGroup })"
          >
            <SelectItem v-for="group in CATEGORY_GROUPS" :key="group" :value="group">{{ t(`categoryGroup.${group}`) }}</SelectItem>
          </Select>
          <Button
            variant="ghost"
            size="icon-sm"
            class="text-muted-foreground"
            :title="category.archived ? t('stores.unarchive') : t('stores.archive')"
            :aria-label="category.archived ? t('stores.unarchive') : t('stores.archive')"
            @click="archive.mutate(category)"
          >
            <component :is="category.archived ? ArchiveRestore : Archive" class="size-4" />
          </Button>
          <ConfirmButton :label="t('categories.manage.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(category.key)">
            <Trash2 class="size-4" />
          </ConfirmButton>
        </div>
      </div>
    </div>
  </Card>
</template>
