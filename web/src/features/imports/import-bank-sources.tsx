import { Landmark } from "lucide-react"

import type { ImportBankAccountSource } from "@/api"
import { Panel, PanelHeader, PanelTitle } from "@/components/primitives"
import { countOf, formatDateTimeStamp } from "@/lib/format"

import { bankSourceLabel } from "./bank-source"

function BankSourceRow({ source }: { source: ImportBankAccountSource }) {
  const label = bankSourceLabel(source)

  return (
    <li
      data-slot="import-bank-source"
      className="flex items-start gap-3 px-[18px] py-3"
    >
      <span className="flex size-8 flex-none items-center justify-center rounded-button bg-surface-2 text-ink-2">
        <Landmark aria-hidden className="size-3.5" strokeWidth={1.8} />
      </span>
      <div className="min-w-0 flex-1">
        <p className="min-w-0 truncate text-[12px] leading-[1.3] font-medium text-ink">
          {label.title}
        </p>
        <p className="mt-1 font-mono text-[10px] leading-[1.4] text-ink-3">
          {label.meta === "" ? "" : `${label.meta} · `}
          added {formatDateTimeStamp(source.created_at)}
        </p>
        {source.recorded_transactions > 0 ? (
          <p className="mt-1 text-[11px] leading-[1.5] text-ink-2">
            Recorded on{" "}
            {countOf(source.recorded_transactions, "ledger transaction")}
          </p>
        ) : null}
      </div>
    </li>
  )
}

export function ImportBankSources({
  sources,
}: {
  sources: readonly ImportBankAccountSource[]
}) {
  if (sources.length === 0) return null

  return (
    <Panel data-slot="import-bank-sources">
      <PanelHeader>
        <PanelTitle>Bank accounts</PanelTitle>
        <span className="text-[11px] leading-none font-medium text-ink-3">
          {countOf(sources.length, "account")}
        </span>
      </PanelHeader>
      <ul
        aria-label="Bank accounts in this import"
        className="divide-y divide-border"
      >
        {sources.map((source) => (
          <BankSourceRow key={source.id} source={source} />
        ))}
      </ul>
    </Panel>
  )
}
