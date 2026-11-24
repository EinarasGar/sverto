import { useState } from "react"

import {
  Panel,
  PanelHeader,
  PanelTitle,
  SegmentedControl,
} from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { toast, TOAST_TIMEOUT_MS } from "@/components/ui/toast"
import { accountLabel } from "@/lib/domain/refs"
import { formatDateStamp } from "@/lib/format"
import type { TransactionId, UserId } from "@/lib/query"

import type { DecideHeldTransactionVariables, HeldTransaction } from "./api"
import { useDecideHeldTransaction } from "./api"
import { TransactionTile } from "./transaction-tile"
import { reviewKeyAction } from "./use-review-keyboard"

function HeldCard({
  held,
  onDecide,
}: {
  held: HeldTransaction
  onDecide: (variables: DecideHeldTransactionVariables) => void
}) {
  const [selectedId, setSelectedId] = useState<TransactionId | null>(null)
  const candidate =
    held.candidates.find(({ row }) => row.transactionId === selectedId) ??
    held.candidates[0] ??
    null

  return (
    <li
      data-slot="held-transaction"
      aria-label={`${held.description} on ${formatDateStamp(held.date)}`}
      className="px-[18px] py-[14px]"
    >
      <div className="grid grid-cols-1 gap-px overflow-hidden rounded-button border border-border bg-border sm:grid-cols-2">
        <TransactionTile
          eyebrow={`From ${held.providerName}`}
          amount={held.amount}
          description={held.description}
          meta={`${accountLabel(held.account)} · ${formatDateStamp(held.date)}`}
        />
        {candidate === null ? (
          <div className="bg-surface px-[13px] py-[11px] text-[11.5px] leading-[1.5] text-pretty text-ink-3">
            The transactions it looked like were deleted or already matched
            other payments from this bank account, so there is nothing left to
            compare it with.
          </div>
        ) : (
          <TransactionTile
            eyebrow="In your ledger"
            control={
              held.candidates.length > 1 ? (
                <SegmentedControl
                  size="sm"
                  label="Compare with"
                  value={candidate.row.transactionId}
                  onValueChange={setSelectedId}
                  options={held.candidates.map(({ row }, index) => ({
                    value: row.transactionId,
                    label: `Match ${String(index + 1)}`,
                  }))}
                />
              ) : null
            }
            amount={candidate.amount}
            description={candidate.row.description.primary}
            meta={`${candidate.row.category?.name ?? "No category"} · ${formatDateStamp(candidate.row.date)}`}
          />
        )}
      </div>
      <div className="mt-[10px] flex flex-wrap gap-2">
        {candidate === null ? (
          <Button
            size="sm"
            onClick={() => {
              onDecide({
                heldTransactionId: held.id,
                decision: { decision: "different" },
              })
            }}
          >
            Add it
          </Button>
        ) : (
          <>
            <Button
              size="sm"
              onClick={() => {
                onDecide({
                  heldTransactionId: held.id,
                  decision: {
                    decision: "same",
                    transaction_id: candidate.row.transactionId,
                  },
                })
              }}
            >
              Same transaction
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                onDecide({
                  heldTransactionId: held.id,
                  decision: { decision: "different" },
                })
              }}
            >
              Different — add it
            </Button>
          </>
        )}
      </div>
    </li>
  )
}

export function HeldTransactionsSection({
  userId,
  held,
}: {
  userId: UserId
  held: readonly HeldTransaction[]
}) {
  const decide = useDecideHeldTransaction(userId)

  if (held.length === 0) return null

  return (
    <Panel
      data-slot="held-transactions"
      className="mb-[22px]"
      onKeyDown={(event) => {
        if (reviewKeyAction(event.nativeEvent) !== null) event.stopPropagation()
      }}
    >
      <PanelHeader className="flex-col items-start gap-[7px]">
        <PanelTitle>Possible duplicates ({held.length})</PanelTitle>
        <p className="text-[11.5px] leading-[1.4] text-ink-3">
          Bank transactions we think you already have
        </p>
      </PanelHeader>
      <ul aria-label="Possible duplicates" className="divide-y divide-border">
        {held.map((row) => (
          <HeldCard
            key={row.id}
            held={row}
            onDecide={(variables) => {
              decide.mutate(variables, {
                onSuccess: (outcome) => {
                  if (outcome === "outdated") {
                    toast.add({
                      type: "info",
                      timeout: TOAST_TIMEOUT_MS,
                      title: "Already changed",
                      description: `“${row.description}” changed since this list loaded. It now shows what is left to decide.`,
                    })
                    return
                  }
                  toast.add({
                    type: "success",
                    timeout: TOAST_TIMEOUT_MS,
                    ...(variables.decision.decision === "same"
                      ? {
                          title: "Same transaction",
                          description: `“${row.description}” is recorded on the transaction you already had.`,
                        }
                      : {
                          title: "Added to your ledger",
                          description: `“${row.description}” is in your ledger as a transaction of its own.`,
                        }),
                  })
                },
              })
            }}
          />
        ))}
      </ul>
    </Panel>
  )
}
