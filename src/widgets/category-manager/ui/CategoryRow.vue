<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Archive, ArchiveRestore, ChevronRight, Trash2 } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { categoryLabel, isEssential, isPassive } from '@/shared/lib/categories'
import { formatMonthKey } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { Switch } from '@/shared/ui/switch'

import { EDITABLE_GROUPS } from '../category-manager.constants'
import type { CategoryActions } from '../model/use-category-actions'

/**
 * One live category. On a desktop: name, group, flag and actions edit in place
 * (Settings-1 · 4). On a phone: name, this month's sum and the flag; a tap
 * opens the editor.
 */
const props = defineProps<{
  category: Category
  actions: CategoryActions
  /** This month's total in the report currency, if the category had spending. */
  total?: string
  currency: string
  /** Name the month next to the sum (the first sum of the list does). */
  showMonth: boolean
  month: string
}>()
const emit = defineEmits<{ edit: [] }>()

const { t } = useI18n()
const phone = useIsPhone()
const label = computed(() => categoryLabel(props.category, t))
const hasFlag = computed(() => props.category.group === 'daily' || props.category.group === 'fixed' || props.category.group === 'income')
const flagOn = computed(() => (props.category.group === 'income' ? isPassive(props.category) : isEssential(props.category)))
const flagText = computed(() => (props.category.group === 'income' ? t('categories.manage.passiveShort') : t('categories.manage.essentialShort')))
const flagLabel = computed(() => (props.category.group === 'income' ? t('categories.manage.passive') : t('categories.manage.essential')))
const monthName = computed(() => formatMonthKey(props.month, { month: 'short' }))
const archiveLabel = computed(() => (props.category.archived ? t('stores.unarchive') : t('stores.archive')))
</script>

<template>
  <Button
    v-if="phone"
    variant="ghost"
    class="h-auto min-h-13 w-full justify-start gap-2 rounded-none border-b px-0 font-normal"
    :class="category.archived && 'opacity-60'"
    data-category-row
    @click="emit('edit')"
  >
    <span class="flex min-w-0 flex-1 flex-col text-left">
      <span class="truncate">{{ label }}</span>
      <span class="flex flex-wrap gap-x-1 text-xs text-muted-foreground">
        <template v-if="total">
          <Money :amount="total" :currency="currency" :decimals="0" />
          <span v-if="showMonth">{{ t('categories.inMonth', { month: monthName }) }}</span>
        </template>
        <span v-if="total && hasFlag && flagOn">·</span>
        <span v-if="hasFlag && flagOn">{{ flagText }}</span>
        <span v-if="category.archived">{{ t('stores.archived') }}</span>
      </span>
    </span>
    <ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
  </Button>

  <div
    v-else
    class="grid min-h-11.5 items-center gap-x-2.5 border-b md:grid-cols-[minmax(0,1fr)_9.5rem_10rem_5rem]"
    :class="category.archived && 'opacity-60'"
    data-category-row
  >
    <Input
      class="h-8"
      :model-value="label"
      :aria-label="t('categories.manage.name')"
      @change="(event: Event) => actions.rename(category, (event.target as HTMLInputElement).value)"
    />
    <Select
      class="h-8"
      :model-value="category.group"
      :aria-label="t('categories.manage.group')"
      @update:model-value="(group) => actions.regroup(category, group as CategoryGroup)"
    >
      <SelectItem v-for="group in EDITABLE_GROUPS" :key="group" :value="group">{{ t(`categoryGroup.${group}`) }}</SelectItem>
    </Select>
    <div v-if="hasFlag" class="flex items-center gap-2 text-sm">
      <Switch
        :model-value="flagOn"
        :title="flagLabel"
        :aria-label="flagLabel"
        :disabled="actions.flag.isPending.value || category.group === 'fixed'"
        @update:model-value="(on) => actions.setFlag(category, on)"
      />
      <span>{{ flagText }}</span>
    </div>
    <span v-else />
    <div class="flex items-center justify-end gap-1">
      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :title="archiveLabel" :aria-label="archiveLabel" @click="actions.archive.mutate(category)">
        <component :is="category.archived ? ArchiveRestore : Archive" class="size-4" />
      </Button>
      <ConfirmButton :label="t('categories.manage.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="actions.remove.mutate(category.key)">
        <Trash2 class="size-4" />
      </ConfirmButton>
    </div>
  </div>
</template>
