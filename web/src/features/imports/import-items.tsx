import { useState } from "react"
import { Link } from "@tanstack/react-router"
import { Lock } from "lucide-react"

import type {
  IdentifiableImportAsk,
  IdentifiableStagedTransaction,
  IdentifiableTransaction,
  ImportAnswer,
  ImportBalanceCheck,
  ImportClassification,
  ImportEnteredBalance,
  ImportItemVerb,
  ImportItemVerbOverride,
  ImportNearMissCandidate,
  ImportSessionStatus,
  ImportUnlistedMovement,
  ImportUnstageableGroup,
  RequiredIdentifiableTransaction,
} from "@/api"
import { Figure } from "@/components/figure"
import type { SegmentedOption } from "@/components/primitives"
import {
  focusRing,
  Panel,
  PanelHeader,
  PanelTitle,
  SegmentedControl,
} from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { LedgerTransactionRow } from "@/features/transactions/api"
import {
  nativeFigureProps,
  toLookupIndex,
  toTransactionRow,
} from "@/features/transactions/api"
import { accountLabel } from "@/lib/domain/refs"
import { countOf, formatDateStamp } from "@/lib/format"
import { cn } from "@/lib/utils"

import type { ImportSessionParams } from "./api"
import { useAnswerAsks, useImportItems, usePinImportItems } from "./api"
import { ImportApply } from "./import-apply"
import { ImportStatusChip } from "./import-status-chip"
import {
  AMEND_REFUSAL_NOTE,
  applyBlocker,
  KIND_NOTE,
  VERB_COPY,
} from "./status"
import { VirtualList } from "./virtual-list"

type VerbFilter = ImportItemVerb | "all"

export type BalanceAsk = Extract<
  IdentifiableImportAsk,
  { kind: "no_checkpoint" }
>

const ITEM_ESTIMATE = 84

const VERB_FILTERS: readonly SegmentedOption<VerbFilter>[] = [
  { value: "all", label: "All" },
  { value: "add", label: "Add" },
  { value: "skip", label: "Skip" },
  { value: "delete", label: "Delete" },
]

function asLedgerTransaction(
  transaction: IdentifiableTransaction
): RequiredIdentifiableTransaction {
  return transaction as RequiredIdentifiableTransaction
}

function UnstageableRow({ group }: { group: ImportUnstageableGroup }) {
  return (
    <li className="flex items-start gap-3 px-[18px] py-3 text-[12px]">
      <span className="w-[72px] flex-none font-mono text-ink-3">
        {formatDateStamp(group.date)}
      </span>
      <div className="min-w-0 flex-1">
        {group.description ? (
          <p className="truncate text-ink">{group.description}</p>
        ) : null}
        <p className="mt-0.5 text-[11px] leading-[1.5] text-pretty text-attention">
          {group.reason}
        </p>
      </div>
    </li>
  )
}

function ItemRow({
  item,
  ledger,
  pinning,
  onPin,
  onKeep,
}: {
  item: IdentifiableStagedTransaction
  ledger: LedgerTransactionRow
  pinning: boolean
  onPin: (verb: ImportItemVerbOverride) => void
  onKeep: (askKey: string) => void
}) {
  const primary = ledger.description.primary
  const date = formatDateStamp(ledger.date)
  const subject = `${primary} on ${date}`
  const verb = VERB_COPY[item.verb]
  const pinnable = item.verb !== "delete"
  const kindNote = KIND_NOTE[item.kind]
  const meta = [
    ledger.typeName,
    ledger.account === null ? null : accountLabel(ledger.account),
    ledger.description.detail,
  ]
    .filter(Boolean)
    .join(" · ")

  return (
    <div
      data-slot="import-item"
      className="grid grid-cols-[72px_minmax(0,1fr)_auto] items-start gap-x-3 gap-y-1.5 px-[18px] py-3 text-[12px]"
    >
      <span className="font-mono text-ink-3">{date}</span>
      <div className={cn("min-w-0", pinnable && "sm:row-span-2")}>
        <p className="truncate text-ink">{primary}</p>
        <p className="mt-0.5 text-[11px] text-ink-3">{meta}</p>
        {kindNote === null ? null : (
          <p className="mt-0.5 text-[11px] text-pretty text-ink-3">
            {kindNote}
          </p>
        )}
        {item.parts.length > 0 ? (
          <p className="mt-0.5 text-[11px] text-pretty text-ink-3">
            Already in your ledger as {countOf(item.parts.length, "row")},
            grouped into one when you apply.
          </p>
        ) : null}
        {item.amend_refusal === null ||
        item.amend_refusal === undefined ? null : (
          <p className="mt-0.5 text-[11px] leading-[1.5] text-pretty text-attention">
            {AMEND_REFUSAL_NOTE[item.amend_refusal]}
          </p>
        )}
        <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
          <ImportStatusChip tone={verb.tone} label={verb.label} />
          {item.verb_locked ? (
            <Lock
              role="img"
              aria-label="Pinned"
              className="size-3 text-ink-3"
            />
          ) : null}
          {item.near_miss_candidates.length > 0 ? (
            <ImportStatusChip tone="attention" label="Possible duplicate" />
          ) : null}
          {isUndecidedStart(item) ? (
            <ImportStatusChip tone="attention" label="To decide" />
          ) : null}
        </div>
      </div>
      <div className="text-right">
        {ledger.primaryAmount === null ? (
          <Figure value={null} />
        ) : (
          <Figure
            {...nativeFigureProps(ledger.primaryAmount)}
            intent={ledger.figureIntent}
          />
        )}
      </div>
      {pinnable ? (
        <div className="col-span-2 col-start-2 flex flex-wrap gap-1 sm:col-span-1 sm:col-start-3 sm:justify-end">
          {item.kind === "joined_transfer" ? (
            <Button
              size="xs"
              variant="ghost"
              aria-label={`Keep apart ${subject}`}
              disabled={pinning}
              onClick={() => {
                onPin("apart")
              }}
            >
              Keep apart
            </Button>
          ) : null}
          {item.verb_locked && item.verb === "skip" ? null : (
            <Button
              size="xs"
              variant="ghost"
              aria-label={`Skip ${subject}`}
              disabled={pinning}
              onClick={() => {
                onPin("skip")
              }}
            >
              Skip
            </Button>
          )}
          {item.verb_locked && item.verb === "add" ? null : (
            <Button
              size="xs"
              variant="ghost"
              aria-label={`Add ${subject}`}
              disabled={pinning}
              onClick={() => {
                onPin("add")
              }}
            >
              Add
            </Button>
          )}
          {item.verb_locked ? (
            <Button
              size="xs"
              variant="ghost"
              aria-label={`Undo pin on ${subject}`}
              disabled={pinning}
              onClick={() => {
                onPin("auto")
              }}
            >
              Undo pin
            </Button>
          ) : null}
        </div>
      ) : null}
      {item.verb === "delete" &&
      item.ask_key !== null &&
      item.ask_key !== undefined ? (
        <div className="col-span-2 col-start-2 flex flex-wrap gap-1 sm:col-span-1 sm:col-start-3 sm:justify-end">
          <Button
            size="xs"
            variant="ghost"
            aria-label={`Keep ${subject}`}
            disabled={pinning}
            onClick={() => {
              if (item.ask_key) onKeep(item.ask_key)
            }}
          >
            Keep
          </Button>
        </div>
      ) : null}
    </div>
  )
}

type Lookup = ReturnType<typeof toLookupIndex>

function unlistedGroups(rows: readonly ImportUnlistedMovement[]) {
  const groups = new Map<string, ImportUnlistedMovement[]>()
  for (const row of rows) {
    const key = row.ask_key ?? row.transaction_id
    groups.set(key, [...(groups.get(key) ?? []), row])
  }
  return [...groups.values()]
}

function accountNames(lookup: Lookup, accountIds: readonly string[]) {
  return accountIds
    .map((accountId) => {
      const account = lookup.accounts.get(accountId)
      return account === undefined ? "another account" : accountLabel(account)
    })
    .join(", ")
}

type NotOnStatementChoice = "delete" | "keep"

function notOnStatementAnswer(choice: NotOnStatementChoice): ImportAnswer {
  const answer = { kind: "not_on_statement", choice }
  return answer as ImportAnswer
}

function UnlistedRows({
  title,
  rows,
  amount,
  lookup,
  answering,
  onAnswer,
}: {
  title: string
  rows: ImportUnlistedMovement[]
  amount: (value: string) => string
  lookup: Lookup
  answering?: boolean
  onAnswer?: (askKey: string, choice: NotOnStatementChoice) => void
}) {
  if (rows.length === 0) return null
  return (
    <div className="mt-1">
      <p className="text-[11px] text-ink-3">{title}</p>
      <ul aria-label={title} className="mt-0.5 flex flex-col gap-1.5">
        {unlistedGroups(rows).map((group) => {
          const first = group[0]
          if (first === undefined) return null
          const askKey = first.ask_key ?? null
          const others = accountNames(lookup, first.other_account_ids)
          const subject =
            group.length > 1
              ? `the ${countOf(group.length, "part")} of ${first.description ?? "this payment"}`
              : [formatDateStamp(first.date), first.description]
                  .filter(Boolean)
                  .join(" ")
          return (
            <li key={askKey ?? first.transaction_id} className="flex flex-col">
              {group.map((row) => (
                <div
                  key={row.transaction_id}
                  className="flex items-center gap-2"
                >
                  <Link
                    to="/transactions"
                    search={{ tx: row.transaction_id }}
                    className={cn(
                      "min-w-0 truncate text-ink underline-offset-2 hover:underline",
                      row.deleting && "text-ink-3 line-through",
                      focusRing.chip
                    )}
                  >
                    {[
                      formatDateStamp(row.date),
                      row.description,
                      amount(row.quantity),
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </Link>
                  {row.explains_difference ? (
                    <ImportStatusChip
                      tone="attention"
                      label="Explains the difference"
                    />
                  ) : null}
                </div>
              ))}
              {group.length > 1 ? (
                <p className="text-[11px] text-ink-3">
                  The parts of one split payment.
                </p>
              ) : null}
              {first.other_account_ids.length === 0 ||
              first.may_book_later ? null : askKey === null ? (
                <p className="text-[11px] text-pretty text-ink-3">
                  A statement or bank feed for {others} shows its other side.
                  Check which account it came from.
                </p>
              ) : (
                <p className="text-[11px] text-pretty text-ink-3">
                  Deleting it also removes it from {others}.
                </p>
              )}
              {onAnswer === undefined || askKey === null ? null : (
                <div className="mt-0.5 flex flex-wrap items-center gap-2">
                  {first.deleting ? (
                    <>
                      <span className="text-[11px] text-negative">
                        Deleted when you apply
                      </span>
                      <Button
                        size="xs"
                        variant="ghost"
                        aria-label={`Keep ${subject}`}
                        disabled={answering}
                        onClick={() => {
                          onAnswer(askKey, "keep")
                        }}
                      >
                        Keep
                      </Button>
                    </>
                  ) : (
                    <Button
                      size="xs"
                      variant="ghost"
                      aria-label={`Delete ${subject} when the import is applied`}
                      disabled={answering}
                      onClick={() => {
                        onAnswer(askKey, "delete")
                      }}
                    >
                      Delete on apply
                    </Button>
                  )}
                </div>
              )}
            </li>
          )
        })}
      </ul>
    </div>
  )
}

function balanceAnswer(quantity: string): ImportAnswer {
  const answer = { kind: "no_checkpoint", choice: "enter_balance", quantity }
  return answer as ImportAnswer
}

function EnterBalanceForm({
  params,
  askKey,
  label,
  onEntered,
}: {
  params: ImportSessionParams
  askKey: string
  label: string
  onEntered?: () => void
}) {
  const answer = useAnswerAsks(params)
  const [value, setValue] = useState("")
  const quantity = value.replace(/\s/g, "")

  return (
    <form
      className="flex flex-wrap items-center gap-2"
      onSubmit={(event) => {
        event.preventDefault()
        if (quantity === "") return
        answer.mutate(
          { answers: [{ askKey, answer: balanceAnswer(quantity) }] },
          { onSuccess: onEntered }
        )
      }}
    >
      <Input
        aria-label={label}
        inputMode="decimal"
        autoComplete="off"
        className="w-[160px] text-right font-mono tabular-nums"
        value={value}
        onChange={(event) => {
          setValue(event.target.value)
        }}
      />
      <Button
        size="sm"
        type="submit"
        disabled={quantity === "" || answer.isPending}
      >
        Check
      </Button>
    </form>
  )
}

function positionName(
  lookup: ReturnType<typeof toLookupIndex>,
  accountId: string,
  assetId: number
) {
  const account = lookup.accounts.get(accountId)
  return {
    account: account === undefined ? "this account" : accountLabel(account),
    ticker: lookup.assets.get(assetId)?.ticker ?? "",
  }
}

function BalanceQuestion({
  params,
  ask,
  lookup,
}: {
  params: ImportSessionParams
  ask: BalanceAsk
  lookup: ReturnType<typeof toLookupIndex>
}) {
  const { account, ticker } = positionName(lookup, ask.account_id, ask.asset_id)
  const date = formatDateStamp(ask.as_of_date)

  return (
    <li className="flex flex-col gap-2 px-[18px] py-3 text-[12px]">
      <div className="flex items-center justify-between gap-3">
        <p className="truncate font-medium text-ink">
          {[account, ticker].filter(Boolean).join(" · ")}
        </p>
        <ImportStatusChip tone="ghost" label="Not checked" />
      </div>
      <p className="text-[11px] leading-[1.5] text-pretty text-ink-3">
        No statement gives a balance to check against. What did {account} hold
        in {ticker} at the end of {date}? Your bank or broker app shows the
        balance after each transaction.
      </p>
      <EnterBalanceForm
        params={params}
        askKey={ask.ask_key}
        label={`Balance of ${account} in ${ticker} at the end of ${date}`}
      />
    </li>
  )
}

interface AdjustmentOffer {
  item: IdentifiableStagedTransaction
  ledger: LedgerTransactionRow
  accountId: string
  assetId: number
  quantity: string
}

function adjustmentOffer(
  item: IdentifiableStagedTransaction,
  lookup: Lookup
): AdjustmentOffer | undefined {
  const { transaction } = item
  if (item.kind !== "adjustment") return undefined
  if (
    transaction.type !== "cash_transfer_in" &&
    transaction.type !== "cash_transfer_out"
  )
    return undefined
  return {
    item,
    ledger: toTransactionRow(asLedgerTransaction(transaction), lookup),
    accountId: transaction.entry.account_id,
    assetId: transaction.entry.asset_id,
    quantity: String(transaction.entry.amount),
  }
}

function BalanceRow({
  params,
  check,
  entered,
  lookup,
  adjustments,
  pinning,
  onPin,
}: {
  params: ImportSessionParams
  check: ImportBalanceCheck
  entered: ImportEnteredBalance | undefined
  lookup: Lookup
  adjustments: readonly AdjustmentOffer[]
  pinning: boolean
  onPin: (itemId: string, verb: ImportItemVerbOverride) => void
}) {
  const [changing, setChanging] = useState(false)
  const answer = useAnswerAsks(params)
  const ownAdjustments = adjustments.filter(
    (offer) =>
      offer.accountId === check.account_id && offer.assetId === check.asset_id
  )
  const { account: name, ticker } = positionName(
    lookup,
    check.account_id,
    check.asset_id
  )
  const account = lookup.accounts.get(check.account_id)
  const amount = (value: string) => `${value} ${ticker}`.trim()
  const offFromStart = Number(check.starting_off_by) !== 0
  const endsBalanced = Number(check.final_off_by) === 0
  const balanced = endsBalanced && !offFromStart && check.changes.length === 0
  const statusLabel = balanced
    ? "Balanced"
    : endsBalanced
      ? "Balanced at the end"
      : `Off by ${amount(check.final_off_by)}`
  const noun = countOf(check.balances_checked, "balance")

  return (
    <li className="flex flex-col gap-1 px-[18px] py-3 text-[12px]">
      <div className="flex items-center justify-between gap-3">
        <p className="truncate font-medium text-ink">
          {[account === undefined ? null : accountLabel(account), ticker]
            .filter(Boolean)
            .join(" · ")}
        </p>
        <ImportStatusChip
          tone={balanced ? "positive" : "attention"}
          label={statusLabel}
        />
      </div>
      <p className="text-[11px] text-ink-3">
        {noun} checked, {formatDateStamp(check.first_date)} to{" "}
        {formatDateStamp(check.last_date)}
      </p>
      {entered === undefined ? null : (
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-ink-3">
          <span>
            Checked against the balance you entered: {amount(entered.quantity)}{" "}
            at the end of {formatDateStamp(entered.date)}.
          </span>
          {changing ? null : (
            <Button
              size="xs"
              variant="ghost"
              aria-label={`Change the balance you entered for ${name}`}
              onClick={() => {
                setChanging(true)
              }}
            >
              Change
            </Button>
          )}
        </div>
      )}
      {entered !== undefined && changing ? (
        <EnterBalanceForm
          params={params}
          askKey={entered.ask_key}
          label={`Balance of ${name} in ${ticker} at the end of ${formatDateStamp(entered.as_of_date)}`}
          onEntered={() => {
            setChanging(false)
          }}
        />
      ) : null}
      {offFromStart ? (
        <p className="text-[11px] text-ink-3">
          Off by {amount(check.starting_off_by)} before the first balance, from
          history before this statement.
        </p>
      ) : null}
      {check.changes.length > 0 ? (
        <ul className="mt-1 flex flex-col gap-1">
          {check.changes.map((change, index) => {
            const day = formatDateStamp(change.through)
            if (change.after === change.through) {
              return (
                <li key={index} className="flex flex-col gap-0.5">
                  <span className="text-attention">
                    {day}: two statements disagree by {amount(change.change)}
                  </span>
                  <span className="text-[11px] text-pretty text-ink-3">
                    Check that both belong to this account. If one doesn't,
                    remove that file and add it again to link it to its own.
                  </span>
                </li>
              )
            }
            const offer = change.explained
              ? undefined
              : ownAdjustments.find(
                  (own) =>
                    own.item.verb !== "add" &&
                    formatDateStamp(own.ledger.date) === day
                )
            return (
              <li key={index} className="flex flex-col gap-0.5">
                <span className="text-attention">
                  {formatDateStamp(change.after)} to {day}:{" "}
                  {amount(change.change)}
                </span>
                {change.awaits_answer ? (
                  <span className="text-[11px] text-pretty text-ink-3">
                    A possible duplicate on these dates is still open. Answer it
                    first; it may settle this.
                  </span>
                ) : null}
                {offer === undefined ? null : (
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-ink-3">
                    <span className="text-pretty">
                      Nothing in your ledger explains this. Upload the statement
                      for these dates, or add an adjustment of{" "}
                      {amount(offer.quantity)}.
                    </span>
                    <Button
                      size="xs"
                      variant="outline"
                      aria-label={`Add an adjustment of ${amount(offer.quantity)} on ${day}`}
                      disabled={pinning}
                      onClick={() => {
                        onPin(offer.item.id, "add")
                      }}
                    >
                      Add adjustment
                    </Button>
                  </div>
                )}
              </li>
            )
          })}
        </ul>
      ) : null}
      {ownAdjustments
        .filter((own) => own.item.verb === "add")
        .map((own) => {
          const day = formatDateStamp(own.ledger.date)
          return (
            <div
              key={own.item.id}
              className="flex flex-wrap items-center gap-x-2 text-[11px] text-ink-3"
            >
              <span>
                Includes the adjustment you added: {amount(own.quantity)} on{" "}
                {day}.
              </span>
              <Button
                size="xs"
                variant="ghost"
                aria-label={`Remove the adjustment of ${amount(own.quantity)} on ${day}`}
                disabled={pinning}
                onClick={() => {
                  onPin(own.item.id, "auto")
                }}
              >
                Remove
              </Button>
            </div>
          )
        })}
      <UnlistedRows
        title="In your ledger but not on this statement"
        rows={check.unlisted.filter((row) => !row.may_book_later)}
        amount={amount}
        lookup={lookup}
        answering={answer.isPending}
        onAnswer={(askKey, choice) => {
          answer.mutate({
            answers: [{ askKey, answer: notOnStatementAnswer(choice) }],
          })
        }}
      />
      <UnlistedRows
        title="In your ledger, dated in the statement's last days: the bank may book these on the next statement"
        rows={check.unlisted.filter((row) => row.may_book_later)}
        amount={amount}
        lookup={lookup}
      />
    </li>
  )
}

function candidateLine(
  candidate: ImportNearMissCandidate,
  lookup: ReturnType<typeof toLookupIndex>
) {
  const account =
    candidate.account_id == null
      ? undefined
      : lookup.accounts.get(candidate.account_id)
  const asset = lookup.assets.get(candidate.asset_id)
  return [
    formatDateStamp(candidate.date),
    candidate.description,
    `${candidate.quantity} ${asset?.ticker ?? ""}`.trim(),
    account === undefined ? null : accountLabel(account),
  ]
    .filter(Boolean)
    .join(" · ")
}

function candidateNote(candidate: ImportNearMissCandidate) {
  if (candidate.parts.length > 0)
    return `${countOf(candidate.parts.length, "row")} in your ledger add up to it. Same groups them into one when you apply.`
  if (!candidate.in_ledger) return "Another file in this import"
  if (candidate.leaves_movement_unrecorded)
    return "Covers only part of this row. The rest won't be recorded."
  return candidate.amend ? "Takes the statement's amount" : null
}

function isUndecidedStart(item: IdentifiableStagedTransaction): boolean {
  return item.kind === "starting_balance" && !item.verb_locked
}

function StartingBalanceRow({
  ledger,
  pinning,
  onPin,
}: {
  ledger: LedgerTransactionRow
  pinning: boolean
  onPin: (verb: ImportItemVerbOverride) => void
}) {
  const account =
    ledger.account === null ? "this account" : accountLabel(ledger.account)
  const date = formatDateStamp(ledger.date)
  const subject = `the starting balance of ${account} on ${date}`
  return (
    <li className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-x-3 gap-y-2 px-[18px] py-3 text-[12px]">
      <div className="min-w-0">
        <p className="font-medium text-ink">
          Start {account} on {date}?
        </p>
        <p className="mt-0.5 text-[11px] text-pretty text-ink-3">
          The statement's first balance needs this before its first row, on top
          of what your ledger already holds. Add it, leave it out, or upload the
          statement before this one.
        </p>
      </div>
      <div className="text-right">
        {ledger.primaryAmount === null ? (
          <Figure value={null} />
        ) : (
          <Figure
            {...nativeFigureProps(ledger.primaryAmount)}
            intent={ledger.figureIntent}
          />
        )}
      </div>
      <div className="col-span-2 flex flex-wrap gap-1">
        <Button
          size="xs"
          variant="outline"
          aria-label={`Add ${subject}`}
          disabled={pinning}
          onClick={() => {
            onPin("add")
          }}
        >
          Add
        </Button>
        <Button
          size="xs"
          variant="ghost"
          aria-label={`Leave out ${subject}`}
          disabled={pinning}
          onClick={() => {
            onPin("skip")
          }}
        >
          Leave out
        </Button>
      </div>
    </li>
  )
}

function NearMissRow({
  item,
  ledger,
  lookup,
  pinning,
  onSame,
  onNew,
}: {
  item: IdentifiableStagedTransaction
  ledger: LedgerTransactionRow
  lookup: ReturnType<typeof toLookupIndex>
  pinning: boolean
  onSame: (candidateId: string) => void
  onNew: () => void
}) {
  const summary = [
    formatDateStamp(ledger.date),
    ledger.description.primary,
    ledger.account === null ? null : accountLabel(ledger.account),
  ]
    .filter(Boolean)
    .join(" · ")
  return (
    <li className="flex flex-col gap-2.5 px-[18px] py-3 text-[12px]">
      <div>
        <p className="font-medium text-ink">Is this already recorded?</p>
        <p className="mt-0.5 text-[11px] text-ink-3">{summary}</p>
      </div>
      <ul className="flex flex-col gap-1.5">
        {item.near_miss_candidates.map((candidate) => {
          const line = candidateLine(candidate, lookup)
          const note = candidateNote(candidate)
          return (
            <li
              key={candidate.id}
              className="flex items-center justify-between gap-3"
            >
              <div className="min-w-0">
                <p className="truncate text-ink">{line}</p>
                {note === null ? null : (
                  <p className="text-[11px] text-ink-3">{note}</p>
                )}
                {candidate.parts.length > 0 ? (
                  <ul
                    aria-label={`Your rows that add up to ${line}`}
                    className="mt-0.5 text-[11px] text-ink-2"
                  >
                    {candidate.parts.map((part) => (
                      <li key={part.id} className="truncate">
                        {[
                          formatDateStamp(part.date),
                          part.description,
                          `${part.quantity} ${lookup.assets.get(candidate.asset_id)?.ticker ?? ""}`.trim(),
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>
              <Button
                size="xs"
                variant="outline"
                aria-label={`Same as ${line}`}
                disabled={pinning}
                onClick={() => {
                  onSame(candidate.id)
                }}
              >
                Same
              </Button>
            </li>
          )
        })}
      </ul>
      <div>
        <Button size="xs" variant="ghost" disabled={pinning} onClick={onNew}>
          It's a new transaction
        </Button>
      </div>
    </li>
  )
}

export function ImportItems({
  params,
  status,
  classification,
  balanceAsks,
}: {
  params: ImportSessionParams
  status: ImportSessionStatus
  classification: ImportClassification
  balanceAsks: readonly BalanceAsk[]
}) {
  const [filter, setFilter] = useState<VerbFilter>("all")
  const classified = classification.status === "classified"
  const query = useImportItems(params, classified)
  const pin = usePinImportItems(params)
  const answer = useAnswerAsks(params)
  const data = query.data
  if (!classified || data?.ready !== true) return null

  const lookup = toLookupIndex(data.lookup_tables)
  const items =
    filter === "all"
      ? data.items
      : data.items.filter((item) => item.verb === filter)
  const balanceChecks = data.balance_checks ?? undefined
  const nearMisses = data.items.filter(
    (item) => item.near_miss_candidates.length > 0
  )
  const undecidedStarts = data.items.filter(isUndecidedStart)
  const adjustments = data.items.flatMap(
    (item) => adjustmentOffer(item, lookup) ?? []
  )
  const uncheckedRest = Math.max(
    0,
    (balanceChecks?.unchecked ?? 0) - balanceAsks.length
  )

  return (
    <Panel data-slot="import-items">
      <PanelHeader className="flex-wrap">
        <PanelTitle>What the import would write</PanelTitle>
        <span className="text-[11px] leading-none font-medium text-ink-3">
          {countOf(items.length, "row")}
        </span>
        <SegmentedControl
          value={filter}
          onValueChange={setFilter}
          options={VERB_FILTERS}
          label="Show rows by verb"
          size="sm"
        />
      </PanelHeader>
      {data.fingerprint === null || data.fingerprint === undefined ? null : (
        <ImportApply
          params={params}
          fingerprint={data.fingerprint}
          items={data.items}
          unstageable={data.unstageable.length}
          checks={balanceChecks?.checks ?? []}
          blocked={
            applyBlocker(status) ??
            (nearMisses.length > 0
              ? "Settle the possible duplicates first."
              : undecidedStarts.length > 0
                ? "Decide the starting balances first."
                : data.items.some(
                      (item) =>
                        item.amend_refusal !== null &&
                        item.amend_refusal !== undefined
                    )
                  ? "Undo the correction that can't be made first."
                  : undefined)
          }
        />
      )}
      {data.read_as_owed.length > 0 ? (
        <ul
          aria-label="Read as money owed"
          className="flex flex-col gap-1 border-b border-border px-[18px] py-3 text-[11px] leading-[1.5] text-pretty text-ink-3"
        >
          {data.read_as_owed.map((accountId) => (
            <li key={accountId}>
              {accountNames(lookup, [accountId])}: the statement printed what
              you owe as a positive amount, so its amounts and balances were
              read as money owed.
            </li>
          ))}
        </ul>
      ) : null}
      {data.unstageable.length > 0 ? (
        <div className="border-b border-border">
          <h3 className="px-[18px] pt-3 text-[9.5px] leading-none font-semibold tracking-[0.1em] text-attention uppercase">
            Couldn't import
          </h3>
          <ul aria-label="Couldn't import" className="divide-y divide-border">
            {data.unstageable.map((group) => (
              <UnstageableRow key={group.id} group={group} />
            ))}
          </ul>
        </div>
      ) : null}
      {balanceChecks !== undefined &&
      (balanceChecks.checks.length > 0 || balanceChecks.unchecked > 0) ? (
        <div className="border-b border-border">
          <h3 className="px-[18px] pt-3 text-[9.5px] leading-none font-semibold tracking-[0.1em] text-ink-3 uppercase">
            Balances
          </h3>
          {balanceChecks.checks.length > 0 ? (
            <ul aria-label="Balances" className="divide-y divide-border">
              {balanceChecks.checks.map((check) => (
                <BalanceRow
                  key={`${check.account_id}:${check.asset_id}`}
                  params={params}
                  check={check}
                  entered={balanceChecks.entered.find(
                    (entered) =>
                      entered.account_id === check.account_id &&
                      entered.asset_id === check.asset_id
                  )}
                  lookup={lookup}
                  adjustments={adjustments}
                  pinning={pin.isPending}
                  onPin={(itemId, verb) => {
                    pin.mutate({ itemIds: [itemId], verb })
                  }}
                />
              ))}
            </ul>
          ) : null}
          {balanceAsks.length > 0 ? (
            <ul
              aria-label="Balances to check"
              className="divide-y divide-border border-t border-border"
            >
              {balanceAsks.map((ask) => (
                <BalanceQuestion
                  key={ask.ask_key}
                  params={params}
                  ask={ask}
                  lookup={lookup}
                />
              ))}
            </ul>
          ) : null}
          {uncheckedRest > 0 ? (
            <p
              className={cn(
                "px-[18px] pb-3 text-[11px] text-ink-3",
                balanceChecks.checks.length === 0 &&
                  balanceAsks.length === 0 &&
                  "pt-2"
              )}
            >
              {countOf(uncheckedRest, "account balance")} couldn't be checked:
              the statements give nothing to compare with.
            </p>
          ) : null}
        </div>
      ) : null}
      {nearMisses.length > 0 ? (
        <div className="border-b border-border">
          <h3 className="px-[18px] pt-3 text-[9.5px] leading-none font-semibold tracking-[0.1em] text-attention uppercase">
            {countOf(nearMisses.length, "possible duplicate")}
          </h3>
          <ul
            aria-label="Possible duplicates"
            className="divide-y divide-border"
          >
            {nearMisses.map((item) => (
              <NearMissRow
                key={item.id}
                item={item}
                ledger={toTransactionRow(
                  asLedgerTransaction(item.transaction),
                  lookup
                )}
                lookup={lookup}
                pinning={pin.isPending}
                onSame={(candidateId) => {
                  pin.mutate({ itemIds: [item.id], verb: "skip", candidateId })
                }}
                onNew={() => {
                  pin.mutate({ itemIds: [item.id], verb: "add" })
                }}
              />
            ))}
          </ul>
        </div>
      ) : null}
      {undecidedStarts.length > 0 ? (
        <div className="border-b border-border">
          <div className="flex items-center justify-between gap-3 px-[18px] pt-3">
            <h3 className="text-[9.5px] leading-none font-semibold tracking-[0.1em] text-attention uppercase">
              {countOf(undecidedStarts.length, "starting balance")} to decide
            </h3>
            {undecidedStarts.length > 1 ? (
              <Button
                size="xs"
                variant="outline"
                disabled={pin.isPending}
                onClick={() => {
                  pin.mutate({
                    itemIds: undecidedStarts.map((item) => item.id),
                    verb: "add",
                  })
                }}
              >
                Add all
              </Button>
            ) : null}
          </div>
          <ul aria-label="Starting balances" className="divide-y divide-border">
            {undecidedStarts.map((item) => (
              <StartingBalanceRow
                key={item.id}
                ledger={toTransactionRow(
                  asLedgerTransaction(item.transaction),
                  lookup
                )}
                pinning={pin.isPending}
                onPin={(verb) => {
                  pin.mutate({ itemIds: [item.id], verb })
                }}
              />
            ))}
          </ul>
        </div>
      ) : null}
      {items.length > 0 ? (
        <VirtualList
          key={filter}
          items={items}
          getKey={(item) => item.id}
          label="What the import would write"
          estimateSize={ITEM_ESTIMATE}
          renderItem={(item) => (
            <ItemRow
              item={item}
              ledger={toTransactionRow(
                asLedgerTransaction(item.transaction),
                lookup
              )}
              pinning={pin.isPending || answer.isPending}
              onPin={(verb) => {
                pin.mutate({ itemIds: [item.id], verb })
              }}
              onKeep={(askKey) => {
                answer.mutate({
                  answers: [{ askKey, answer: notOnStatementAnswer("keep") }],
                })
              }}
            />
          )}
        />
      ) : (
        <p className="px-[18px] py-3 text-[12px] text-ink-3">
          No rows to show.
        </p>
      )}
    </Panel>
  )
}
