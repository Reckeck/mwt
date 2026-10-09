export const LIABILITY_KINDS: LiabilityKind[] = ['credit_card', 'loan', 'mortgage', 'installment', 'other']

export function emptyLiabilityTerms(): LiabilityTerms {
  return { kind: 'credit_card', rate: null, limit: null, due_day: null, monthly_payment: null }
}
