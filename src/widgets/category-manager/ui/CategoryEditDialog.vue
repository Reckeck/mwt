<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Archive, ArchiveRestore, Trash2 } from 'lucide-vue-next'

import { categoryLabel, isEssential, isPassive } from '@/shared/lib/categories'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

import { EDITABLE_GROUPS } from '../category-manager.constants'
import type { CategoryActions } from '../model/use-category-actions'

/** The phone's editor of one category: the same fields a desktop row edits in place. */
const props = defineProps<{ category: Category | null; actions: CategoryActions }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const name = ref('')
watch(
  () => props.category?.key,
  () => {
    name.value = props.category ? categoryLabel(props.category, t) : ''
  },
  { immediate: true },
)

const hasFlag = computed(() => props.category && props.category.group !== 'savings')
const flagOn = computed(() =>
  props.category?.group === 'income' ? isPassive(props.category) : props.category ? isEssential(props.category) : false,
)
// Template closures lose the v-if narrowing, so actions get the category through this guard.
function withCategory(run: (category: Category) => void) {
  if (props.category) run(props.category)
}
const flagLabel = computed(() => (props.category?.group === 'income' ? t('categories.manage.passive') : t('categories.manage.essential')))

function done(action: () => void) {
  action()
  open.value = false
}
</script>

<template>
  <Dialog overlay="category-edit" v-model:open="open" :title="category ? categoryLabel(category, t) : ''" :description="t('categories.manage.hint')">
    <div v-if="category" class="flex flex-col gap-4">
      <Input
        v-model="name"
        class="touch-hit"
        :aria-label="t('categories.manage.name')"
        @change="actions.rename(category, name)"
      />
      <Select
        :model-value="category.group"
        :aria-label="t('categories.manage.group')"
        @update:model-value="(group) => withCategory((item) => actions.regroup(item, group as CategoryGroup))"
      >
        <SelectItem v-for="group in EDITABLE_GROUPS" :key="group" :value="group">{{ t(`categoryGroup.${group}`) }}</SelectItem>
      </Select>
      <SwitchRow
        id="category-flag"
        v-if="hasFlag"
        :label="flagLabel"
        :model-value="flagOn"
        :disabled="actions.flag.isPending.value || category.group === 'fixed'"
        @update:model-value="(on: boolean) => withCategory((item) => actions.setFlag(item, on))"
      />
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" class="touch-hit" @click="done(() => withCategory((item) => actions.archive.mutate(item)))">
          <component :is="category.archived ? ArchiveRestore : Archive" class="size-4" />
          {{ category.archived ? t('stores.unarchive') : t('stores.archive') }}
        </Button>
        <ConfirmButton
          :label="t('categories.manage.delete')"
          :confirm-label="t('actions.confirmDelete')"
          @confirm="done(() => withCategory((item) => actions.remove.mutate(item.key)))"
        >
          <Trash2 class="size-4" />
        </ConfirmButton>
      </div>
    </div>
  </Dialog>
</template>
