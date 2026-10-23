<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'

import { FormField } from '@/shared/ui/form-field'

/**
 * «Пополнение» in «Изменить цель» (New-Goal 4): what fills the goal is an
 * automation, so it is changed there, not in the goal's form.
 */
const props = defineProps<{ filler: RelationItem | null }>()

const { t } = useI18n()
const to = computed(() => ({ name: props.filler?.target.kind === 'workflow' ? 'automations-workflows' : 'automations' }))
</script>

<template>
  <FormField :label="t('goal.filler')" data-goal-edit-filler>
    <div class="flex min-h-9 flex-wrap items-center gap-x-3 gap-y-1 text-sm">
      <span class="min-w-0 flex-1" :class="{ 'text-muted-foreground': !props.filler }">{{ props.filler?.label ?? t('goal.fillerNone') }}</span>
      <RouterLink :to="to" class="text-primary hover:underline pointer-coarse:py-2">{{ t('goal.fillerEdit') }}</RouterLink>
    </div>
  </FormField>
</template>
