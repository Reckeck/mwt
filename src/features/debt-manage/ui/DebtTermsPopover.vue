<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { call } from '@/shared/api/ipc'
import { formatDayMonthShort } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { cn } from '@/shared/lib/utils'

import type { DebtTermsPopoverProps } from '../debt-manage.types'

/**
 * «вернуть до 20 нояб» in the debt line (Ops-1 · 6): the term reads as text and
 * opens a small form for the date and a note. With no term yet it says «срок».
 */
const props = defineProps<DebtTermsPopoverProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

const terms = useQuery({
  // no placeholder: another contact's terms must not show under this one
  queryKey: computed(() => ['debt-terms', props.contactId]),
  queryFn: () => call<DebtTerms[]>('list_debt_terms', { contactId: props.contactId }),
})
const own = computed(() => terms.data.value?.find((entry) => entry.asset === props.asset.toUpperCase()) ?? null)
const overdue = computed(() => Boolean(own.value?.due_at && own.value.due_at < localDate(new Date())))

const open = ref(false)
const due = ref<string | null>('')
const note = ref('')
watch(open, (now) => {
  if (!now) return
  due.value = own.value?.due_at ?? ''
  note.value = own.value?.note ?? ''
})

const save = useMutation({
  mutationFn: () =>
    call<DebtTerms | null>('set_debt_terms', {
      input: { contact_id: props.contactId, asset: props.asset, due_at: due.value || null, note: note.value.trim() },
    }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['debt-terms'] })
    void queryClient.invalidateQueries({ queryKey: ['upcoming'] })
    open.value = false
  },
  meta: { success: 'debt.termsSaved', error: 'debt.termsFailed' },
})

const label = computed(() => {
  const date = own.value?.due_at
  if (!date) return t('debt.setTerm')
  return overdue.value
    ? t('debt.overdueShort', { date: formatDayMonthShort(date) })
    : t('debt.dueShort', { date: formatDayMonthShort(date) })
})
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger
      data-touch-inline
      :class="
        cn(
          'rounded-sm underline decoration-dotted underline-offset-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          overdue ? 'text-destructive' : 'text-muted-foreground hover:text-foreground',
        )
      "
      :title="own?.note || undefined"
    >
      {{ label }}
    </PopoverTrigger>
    <PopoverContent overlay="debt-terms" align="start" class="grid w-[min(18rem,calc(100vw-2rem))] gap-3">
      <form class="grid gap-3" @submit.prevent="save.mutate()">
        <FormField :label="t('debt.dueDate')">
          <DateTimePicker v-model="due" :aria-label="t('debt.dueDate')" />
        </FormField>
        <FormField for="debt-terms-note" :label="t('debt.note')">
          <Input id="debt-terms-note" v-model="note" :placeholder="t('debt.notePlaceholder')" />
        </FormField>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="outline" size="sm" @click="open = false">{{ t('actions.cancel') }}</Button>
          <Button type="submit" size="sm" :loading="save.isPending.value">{{ t('actions.save') }}</Button>
        </div>
      </form>
    </PopoverContent>
  </Popover>
</template>
