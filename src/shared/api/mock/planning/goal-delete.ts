/** `delete_goal` / `restore_goal` (`service/goals.rs`): envelopes go back to «можно потратить», savers stop. */

import { mockEnvelopes } from '../catalog'
import { workflows } from '../workflows/state'
import { goals } from './state'

/** Workflows that send money into a wallet only this goal saves in. */
function savers(goal: Goal): Workflow[] {
  const shared = new Set(goals.filter((other) => other.id !== goal.id).flatMap((other) => other.scope.store_ids))
  const stores = goal.scope.store_ids.filter((store) => !shared.has(store))
  return workflows.filter((flow) => flow.active && stores.some((store) => flow.graph.includes(`"to":"${store}"`)))
}

export function deleteGoal(id: string): GoalDeleted | null {
  const index = goals.findIndex((goal) => goal.id === id)
  if (index < 0) return null
  const [goal] = goals.splice(index, 1)
  const envelopes = mockEnvelopes.filter((envelope) => envelope.goal_id === id)
  for (const envelope of envelopes) mockEnvelopes.splice(mockEnvelopes.indexOf(envelope), 1)
  const stopped = savers(goal)
  for (const flow of stopped) flow.active = false
  return { goal, envelopes, workflow_ids: stopped.map((flow) => flow.id) }
}

export function restoreGoal(deleted: GoalDeleted): Goal {
  goals.push(deleted.goal)
  mockEnvelopes.push(...deleted.envelopes)
  for (const flow of workflows) if (deleted.workflow_ids.includes(flow.id)) flow.active = true
  return deleted.goal
}
