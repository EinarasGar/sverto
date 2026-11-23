import { useState } from "react"

import type {
  IdentifiableStagedTransaction,
  ImportBalanceCheck,
  ImportItemVerb,
} from "@/api"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { toast } from "@/components/ui/toast"
import { countOf } from "@/lib/format"

import type { ImportSessionParams } from "./api"
import { useApplyImportSession } from "./api"
import type { SourceCopy } from "./status"

function writeSummary(
  items: IdentifiableStagedTransaction[],
  unstageable: number,
  sourceCopy: SourceCopy
): string {
  const count = (verb: ImportItemVerb) =>
    items.filter((item) => item.verb === verb).length
  const starting = items.filter(
    (item) => item.kind === "starting_balance" && item.verb === "add"
  ).length
  const parts = [
    count("add") > 0
      ? `adds ${countOf(count("add"), "transaction")}${
          starting > 0
            ? ` (${countOf(starting, "starting balance")} ${sourceCopy.startingBalancesFrom})`
            : ""
        }`
      : null,
    count("amend") > 0 ? `corrects ${count("amend")}` : null,
    count("delete") > 0 ? `deletes ${count("delete")}` : null,
  ].filter((part) => part !== null)
  const skipped = count("skip")
  const writes =
    parts.length === 0 ? "Nothing new to write" : `Applying ${parts.join(", ")}`
  const summary =
    skipped > 0
      ? `${writes}; ${countOf(skipped, "row")} already in your ledger.`
      : `${writes}.`
  return unstageable > 0
    ? `${summary} ${countOf(unstageable, "group")} that can't be imported will be left out.`
    : summary
}

function isOff(check: ImportBalanceCheck): boolean {
  return (
    Number(check.starting_off_by) !== 0 ||
    Number(check.final_off_by) !== 0 ||
    check.changes.length > 0
  )
}

export function ImportApply({
  params,
  fingerprint,
  items,
  unstageable,
  checks,
  sourceCopy,
  blocked,
}: {
  params: ImportSessionParams
  fingerprint: string
  items: IdentifiableStagedTransaction[]
  unstageable: number
  checks: ImportBalanceCheck[]
  sourceCopy: SourceCopy
  blocked: string | undefined
}) {
  const [confirming, setConfirming] = useState(false)
  const apply = useApplyImportSession(params)
  const summary = writeSummary(items, unstageable, sourceCopy)
  const off = checks.filter(isOff).length

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-[18px] py-3">
      <p className="min-w-0 text-[12px] leading-[1.5] text-pretty text-ink-2">
        {blocked ?? summary}
      </p>
      <Button
        size="sm"
        disabled={blocked !== undefined || apply.isPending}
        onClick={() => {
          setConfirming(true)
        }}
      >
        Apply to ledger
      </Button>
      <AlertDialog
        open={confirming}
        onOpenChange={(next) => {
          if (!apply.isPending) setConfirming(next)
        }}
      >
        <AlertDialogContent className="rounded-sheet border border-border-strong bg-surface sm:max-w-md">
          <AlertDialogHeader>
            <AlertDialogTitle className="text-[14px] leading-[1.3] font-semibold">
              Apply this import?
            </AlertDialogTitle>
          </AlertDialogHeader>
          <div className="rounded-panel border border-border px-[18px] pt-4 pb-[17px]">
            <AlertDialogDescription className="text-[11.5px] leading-[1.6] text-pretty text-ink-2 sm:text-left">
              {summary}
            </AlertDialogDescription>
            {off > 0 ? (
              <p className="mt-[7px] text-[11.5px] leading-[1.6] text-pretty text-attention sm:text-left">
                {countOf(off, "account")} {sourceCopy.balancesOff}
              </p>
            ) : null}
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={apply.isPending}>
              Keep reviewing
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={apply.isPending}
              onClick={() => {
                apply.mutate(
                  { fingerprint },
                  {
                    onSuccess: () => {
                      toast.add({
                        type: "success",
                        timeout: 6000,
                        title: "Import applied",
                      })
                    },
                    onSettled: () => {
                      setConfirming(false)
                    },
                  }
                )
              }}
            >
              {apply.isPending ? "Applying…" : "Apply"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
