import { useState } from "react"

import type { MergeQuickUploadRequest } from "@/api"
import { SegmentedControl } from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { accountLabel } from "@/lib/domain/refs"
import { formatDateStamp } from "@/lib/format"
import type { QuickUploadId, TransactionId, UserId } from "@/lib/query"

import type { ReceiptMergeDraft } from "./api"
import { useQuickUploadMatches } from "./api"
import { TransactionTile } from "./transaction-tile"
import { reviewKeyAction } from "./use-review-keyboard"

const MERGE_UNAVAILABLE =
  "Myra's reading of this receipt is missing a date, a category or an amount, so it cannot be merged."

export function ReceiptMatches({
  userId,
  quickUploadId,
  draft,
  isBusy,
  onMerge,
}: {
  userId: UserId
  quickUploadId: QuickUploadId
  draft: ReceiptMergeDraft | null
  isBusy: boolean
  onMerge: (request: MergeQuickUploadRequest) => void
}) {
  const matches = useQuickUploadMatches(userId, quickUploadId)
  const [selectedId, setSelectedId] = useState<TransactionId | null>(null)
  const match =
    matches.find((row) => row.transactionId === selectedId) ?? matches[0]

  if (match === undefined) return null

  return (
    <section
      data-slot="receipt-matches"
      aria-label="Already in your ledger"
      className="mt-[15px]"
      onKeyDown={(event) => {
        if (reviewKeyAction(event.nativeEvent) !== null) event.stopPropagation()
      }}
    >
      <div className="overflow-hidden rounded-button border border-border">
        <TransactionTile
          eyebrow="Already in your ledger"
          control={
            matches.length > 1 ? (
              <SegmentedControl
                size="sm"
                label="Merge into"
                value={match.transactionId}
                onValueChange={setSelectedId}
                options={matches.map((row, index) => ({
                  value: row.transactionId,
                  label: `Match ${String(index + 1)}`,
                }))}
              />
            ) : null
          }
          amount={match.primaryAmount}
          description={match.description.primary}
          meta={`${match.account === null ? "No account" : accountLabel(match.account)} · ${formatDateStamp(match.date)}`}
        />
      </div>
      <div className="mt-[10px] flex flex-wrap items-center gap-x-3 gap-y-2">
        <Button
          size="sm"
          disabled={draft === null || isBusy}
          {...(draft === null ? { title: MERGE_UNAVAILABLE } : {})}
          onClick={() => {
            if (draft === null) return
            onMerge({ ...draft, transaction_id: match.transactionId })
          }}
        >
          Merge into it
        </Button>
        <p className="text-[11px] leading-[1.5] text-pretty text-ink-3">
          The receipt&rsquo;s lines and date replace it; its account stays.
        </p>
      </div>
    </section>
  )
}
