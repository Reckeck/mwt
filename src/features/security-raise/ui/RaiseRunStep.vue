<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Circle, X } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Spinner } from '@/shared/ui/spinner'

import { RAISE_STAGES, type RaiseFailure } from '../security-raise.types'

/**
 * Step 4: the three stages of the re-encryption (backup, re-encryption, check) with the one under way
 * marked, or, after a refusal, where it stopped and what is left intact.
 */
const props = defineProps<{ level: SecurityLevel; stage: RaiseStage | null; failure: RaiseFailure | null }>()
const { t } = useI18n()

/** Index of the stage that is running, or the one that failed. */
const at = computed(() => RAISE_STAGES.indexOf(props.failure?.stage ?? props.stage ?? 'backup'))
type Status = 'done' | 'active' | 'failed' | 'pending'
const status = (index: number): Status =>
  index < at.value ? 'done' : index > at.value ? 'pending' : props.failure ? 'failed' : 'active'

/** `Saving the backup` while it runs, `Backup saved` once it is done, the refusal's own line when it failed. */
const label = (index: number, stage: RaiseStage) =>
  t(
    status(index) === 'failed'
      ? `securityRaise.failure.stage.${stage}`
      : status(index) === 'done'
        ? `securityRaise.stage.${stage}`
        : `securityRaise.stageActive.${stage}`,
  )
</script>

<template>
  <div class="grid gap-4">
    <Alert v-if="props.failure" variant="destructive">
      <X />
      <AlertTitle>{{ t('securityRaise.failure.title') }}</AlertTitle>
      <AlertDescription>
        {{
          props.failure.stage === 'backup'
            ? t('securityRaise.failure.textBackup')
            : t('securityRaise.failure.text', { level: props.level === 'pin' ? 2 : 1 })
        }}
      </AlertDescription>
    </Alert>
    <p v-else class="text-sm">{{ t('securityRaise.run.lead') }}</p>
    <ul class="grid gap-2 text-sm" data-raise-stages>
      <li v-for="(stage, index) in RAISE_STAGES" :key="stage" class="flex items-center gap-2" :data-status="status(index)">
        <!-- One 16px slot for every state: a stage changing status moves nothing. -->
        <span class="flex size-4 shrink-0 items-center justify-center" aria-hidden="true">
          <Check v-if="status(index) === 'done'" class="size-4 text-success" />
          <Spinner v-else-if="status(index) === 'active'" class="text-primary" />
          <X v-else-if="status(index) === 'failed'" class="size-4 text-destructive" />
          <Circle v-else class="size-3 text-muted-foreground" />
        </span>
        <span :class="status(index) === 'pending' ? 'text-muted-foreground' : ''">
          {{ label(index, stage) }}
        </span>
      </li>
    </ul>
  </div>
</template>
