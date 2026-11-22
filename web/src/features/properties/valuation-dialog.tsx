import { useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { isSameDay, isToday } from "date-fns"

import { useUserId } from "@/auth"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { toast } from "@/components/ui/toast"
import { userAssetPairQueryOptions } from "@/features/assets/api"
import { impliedChangeLine } from "@/features/assets/valuation"
import {
  formatEditorDate,
  parseEditorDate,
} from "@/features/transactions/editor/date-input"
import { formatDateStamp, formatMoney } from "@/lib/format"

import type { Property, Valuation } from "./api"
import { useAddPropertyValuation, useEditPropertyValuation } from "./api"
import {
  DATE_IN_FUTURE,
  DATE_UNREADABLE,
  isAfterNow,
  parsePositiveAmount,
  VALUE_REQUIRED,
} from "./property-form"

const DATE_HINT = "Plain English works — today, yesterday, 2026-02-14."
const CONFLICT =
  "There is already a valuation at that exact time. Pick another date."

// Creating a property stores today's value at the moment of creation, so a value entered
// for today at midnight would sort before it and never become the latest. An edit that keeps
// the day keeps the original time, so changing only the amount does not move the valuation.
function valuedAt(
  daySeconds: number,
  now: Date,
  original: Valuation | null
): number {
  const day = new Date(daySeconds * 1000)
  if (original !== null && isSameDay(day, new Date(original.date * 1000))) {
    return original.date
  }
  return isToday(day) ? Math.floor(now.getTime() / 1000) : daySeconds
}

function stamp(seconds: number): string {
  return formatDateStamp(new Date(seconds * 1000), { year: "always" })
}

export function ValuationDialog({
  property,
  currency,
  valuation = null,
  onOpenChange,
}: {
  property: Property
  currency: string
  valuation?: Valuation | null
  onOpenChange: (open: boolean) => void
}) {
  const userId = useUserId()
  const add = useAddPropertyValuation(userId)
  const edit = useEditPropertyValuation(userId)
  const pair = useQuery({
    ...userAssetPairQueryOptions(
      userId,
      property.assetId,
      property.currencyAssetId
    ),
    enabled: valuation === null,
  })
  const latest = pair.data?.quote ?? null
  const [dateText, setDateText] = useState(
    valuation === null ? "today" : formatEditorDate(valuation.date)
  )
  const [valueText, setValueText] = useState(
    valuation === null ? "" : String(valuation.value)
  )
  const [issues, setIssues] = useState<{ date?: string; value?: string }>({})

  const parsedDate = useMemo(
    () => parseEditorDate(dateText, new Date()),
    [dateText]
  )
  const value = parsePositiveAmount(valueText)
  const implied =
    valuation !== null || value === null
      ? null
      : impliedChangeLine(value, latest?.rate ?? null)
  const pending = add.isPending || edit.isPending
  const conflict =
    add.error?.kind === "conflict" || edit.error?.kind === "conflict"

  function submit() {
    const now = new Date()
    const dateIssue =
      parsedDate.date === null
        ? DATE_UNREADABLE
        : isAfterNow(parsedDate.date, now)
          ? DATE_IN_FUTURE
          : null
    setIssues({
      ...(dateIssue === null ? {} : { date: dateIssue }),
      ...(value === null ? { value: VALUE_REQUIRED } : {}),
    })
    if (parsedDate.date === null || dateIssue !== null || value === null) {
      return
    }

    const next = { date: valuedAt(parsedDate.date, now, valuation), value }
    const pairIds = {
      assetId: property.assetId,
      currencyAssetId: property.currencyAssetId,
    }
    const onSuccess = () => {
      onOpenChange(false)
      toast.add({
        type: "success",
        title: valuation === null ? "Valuation added" : "Valuation saved",
        description: `${property.name} was worth ${formatMoney(value, { currency })} on ${stamp(next.date)}.`,
      })
    }
    if (valuation === null) {
      add.mutate({ ...pairIds, valuation: next }, { onSuccess })
    } else {
      edit.mutate({ ...pairIds, from: valuation, to: next }, { onSuccess })
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {valuation === null ? "Add valuation" : "Edit valuation"}
          </DialogTitle>
          <DialogDescription>
            What the whole of {property.name} is worth, in {currency}. Every
            total that holds it uses the latest valuation on or before the date
            in question.
          </DialogDescription>
        </DialogHeader>
        <form
          id="property-valuation-form"
          noValidate
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <Field data-invalid={issues.value !== undefined}>
            <FieldLabel htmlFor="property-valuation-value">
              Value in {currency}
            </FieldLabel>
            <Input
              id="property-valuation-value"
              inputMode="decimal"
              autoComplete="off"
              placeholder="0.00"
              value={valueText}
              onChange={(event) => {
                setValueText(event.target.value)
              }}
            />
            <FieldDescription>
              {valuation !== null
                ? `Was ${formatMoney(valuation.value, { currency })} on ${stamp(valuation.date)}.`
                : latest === null
                  ? "This is the first valuation for it."
                  : `Last valued at ${formatMoney(latest.rate, { currency })} on ${formatDateStamp(latest.asOf, { year: "always" })}.`}
            </FieldDescription>
            {issues.value ? <FieldError>{issues.value}</FieldError> : null}
          </Field>

          <Field data-invalid={issues.date !== undefined}>
            <FieldLabel htmlFor="property-valuation-date">Valued on</FieldLabel>
            <Input
              id="property-valuation-date"
              value={dateText}
              autoComplete="off"
              onChange={(event) => {
                setDateText(event.target.value)
              }}
            />
            <FieldDescription>{parsedDate.label ?? DATE_HINT}</FieldDescription>
            {issues.date ? <FieldError>{issues.date}</FieldError> : null}
          </Field>

          {implied === null ? null : (
            <p className="rounded-md border border-dashed border-border-strong px-[13px] py-[10px] text-[11px] leading-[1.5] text-pretty text-ink-3">
              {implied}
            </p>
          )}

          {conflict ? (
            <p className="text-[11.5px] leading-[1.5] text-pretty text-negative">
              {CONFLICT}
            </p>
          ) : null}
        </form>
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            Cancel
          </Button>
          <Button
            type="submit"
            form="property-valuation-form"
            disabled={pending}
          >
            {pending ? "Saving…" : "Save valuation"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
