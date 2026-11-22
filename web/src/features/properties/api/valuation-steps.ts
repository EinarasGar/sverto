export interface Valuation {
  date: number
  value: number
}

export type ValuationStep =
  { kind: "add"; valuation: Valuation } | { kind: "delete"; date: number }

// A valuation's time is unique per asset, so a move within the same second must free the
// slot first; any other edit adds first, so a failed add leaves the old valuation in place.
export function valuationEditSteps(
  from: Valuation,
  to: Valuation
): ValuationStep[] {
  const remove: ValuationStep = { kind: "delete", date: from.date }
  const add: ValuationStep = { kind: "add", valuation: to }
  return Math.abs(to.date - from.date) <= 1 ? [remove, add] : [add, remove]
}
