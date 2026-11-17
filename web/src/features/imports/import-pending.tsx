import type { IdentifiableImportPendingTransaction } from "@/api"
import { Panel, PanelHeader, PanelTitle } from "@/components/primitives"
import { countOf, formatDateStamp } from "@/lib/format"

import type { ImportSessionParams } from "./api"
import { useImportPending } from "./api"
import { askSubject, questionAnchor } from "./link-copy"
import { VirtualList } from "./virtual-list"

const ROW_ESTIMATE = 76

function PendingRow({ row }: { row: IdentifiableImportPendingTransaction }) {
  return (
    <div className="flex items-start gap-3 px-[18px] py-3 text-[12px]">
      <span className="w-[72px] flex-none font-mono text-ink-3">
        {formatDateStamp(row.date)}
      </span>
      <div className="min-w-0 flex-1">
        <p className="truncate text-ink">
          {row.description ?? row.account_ref}
        </p>
        <p className="mt-0.5 text-[11px] text-ink-3">{row.account_ref}</p>
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          {row.blocked_by.map((key) => {
            const subject = askSubject(key)
            return (
              <button
                key={key}
                type="button"
                className="rounded-chip bg-surface-2 px-1.5 py-0.5 text-[10.5px] text-ink-2 hover:text-ink"
                onClick={() => {
                  document
                    .getElementById(questionAnchor(subject.kind, subject.name))
                    ?.scrollIntoView({ behavior: "smooth", block: "center" })
                }}
              >
                Waiting on “{subject.name}”
              </button>
            )
          })}
        </div>
      </div>
      <div className="flex-none text-right font-mono text-ink tabular-nums">
        {row.legs.map((leg, index) => (
          <div key={index}>
            {leg.quantity} {leg.asset_ref}
          </div>
        ))}
      </div>
    </div>
  )
}

export function ImportPending({
  params,
  pending,
}: {
  params: ImportSessionParams
  pending: number
}) {
  const query = useImportPending(params, pending > 0)
  if (pending === 0) return null

  return (
    <Panel data-slot="import-pending">
      <PanelHeader>
        <PanelTitle>Waiting on your answers</PanelTitle>
        <span className="text-[11px] leading-none font-medium text-ink-3">
          {countOf(pending, "row")}
        </span>
      </PanelHeader>
      <VirtualList
        items={query.data ?? []}
        getKey={(row) => row.id}
        label="Rows waiting on your answers"
        estimateSize={ROW_ESTIMATE}
        renderItem={(row) => <PendingRow row={row} />}
      />
    </Panel>
  )
}
