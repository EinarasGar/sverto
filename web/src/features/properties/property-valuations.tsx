import { useMemo, useState } from "react"
import { useSuspenseQuery } from "@tanstack/react-query"
import { Link } from "@tanstack/react-router"
import { Plus, Trash2 } from "lucide-react"

import { useUserId } from "@/auth"
import { Figure } from "@/components/figure"
import {
  Footnote,
  PageHeader,
  PageHeaderBackLink,
  Panel,
  ROW_ACTIVATION_CLASS,
} from "@/components/primitives"
import { EmptyState } from "@/components/states/empty-state"
import { Button } from "@/components/ui/button"
import { toast } from "@/components/ui/toast"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { userAssetPairRatesQueryOptions } from "@/features/assets/api"
import { ConfirmDestructive } from "@/features/settings/confirm-dialog"
import { countOf, formatDateStamp, formatMoney } from "@/lib/format"
import { cn } from "@/lib/utils"

import type { Property, Valuation } from "./api"
import { useDeletePropertyValuation, usePropertiesSuspense } from "./api"
import { PROPERTY_KIND_ICONS, PROPERTY_KIND_LABELS } from "./presentation"
import { PropertyNotFound } from "./property-not-found"
import { usePropertyCurrency } from "./use-property-currency"
import { ValuationDialog } from "./valuation-dialog"

type ValuationsDialog =
  | { kind: "add" }
  | { kind: "edit"; valuation: Valuation }
  | { kind: "delete"; valuation: Valuation }

function stamp(valuation: Valuation): string {
  return formatDateStamp(new Date(valuation.date * 1000), { year: "always" })
}

function ValuationsScreen({ property }: { property: Property }) {
  const userId = useUserId()
  const currency = usePropertyCurrency(property)
  const remove = useDeletePropertyValuation(userId)
  const rates = useSuspenseQuery(
    userAssetPairRatesQueryOptions(
      userId,
      property.assetId,
      property.currencyAssetId,
      "all"
    )
  ).data
  const valuations = useMemo(
    () =>
      rates
        .map((point): Valuation => ({
          date: point.date.getTime() / 1000,
          value: point.rate,
        }))
        .sort((a, b) => b.date - a.date),
    [rates]
  )
  const [dialog, setDialog] = useState<ValuationsDialog | null>(null)
  const KindIcon = PROPERTY_KIND_ICONS[property.kind]
  const money = (value: number) =>
    currency === null ? String(value) : formatMoney(value, { currency })

  const closeDialog = (open: boolean) => {
    if (!open) setDialog(null)
  }
  const openAdd = () => {
    setDialog({ kind: "add" })
  }
  const deleting = dialog?.kind === "delete" ? dialog.valuation : null

  return (
    <>
      <PageHeader
        back={
          <PageHeaderBackLink
            render={
              <Link
                to="/properties/$propertyId"
                params={{ propertyId: property.propertyId }}
              />
            }
          >
            {property.name}
          </PageHeaderBackLink>
        }
        eyebrow={
          <span className="inline-flex items-center gap-[6px]">
            <KindIcon aria-hidden className="size-3" />
            {PROPERTY_KIND_LABELS[property.kind]}
          </span>
        }
        title="Valuations"
        meta={countOf(valuations.length, "valuation")}
        actions={
          <Button size="lg" disabled={currency === null} onClick={openAdd}>
            <Plus aria-hidden data-icon="inline-start" />
            Add valuation
          </Button>
        }
      />

      {valuations.length === 0 ? (
        <EmptyState
          headline="No valuations yet"
          body={`${property.name} counts as nothing in your totals until it has a value.`}
          actions={[
            { label: "Add valuation", kind: "primary", onClick: openAdd },
          ]}
        />
      ) : (
        <div className="flex flex-col gap-[10px]">
          <Panel data-slot="property-valuations">
            <ul className="list-none">
              {valuations.map((valuation) => (
                <li
                  key={valuation.date}
                  data-slot="property-valuation-row"
                  className="flex items-center border-b border-border last:border-b-0"
                >
                  <button
                    type="button"
                    aria-label={`Edit the ${money(valuation.value)} valuation from ${stamp(valuation)}`}
                    className={cn(
                      "flex min-h-[52px] min-w-0 flex-1 items-center gap-3 py-[10px] ps-[18px] pe-3 text-start",
                      ROW_ACTIVATION_CLASS
                    )}
                    onClick={() => {
                      setDialog({ kind: "edit", valuation })
                    }}
                  >
                    <span className="min-w-0 flex-1 text-[13px] leading-[1.3] font-medium">
                      {stamp(valuation)}
                    </span>
                    {currency === null ? null : (
                      <Figure
                        value={valuation.value}
                        currency={currency}
                        size="base"
                        className="flex-none text-[14px] font-semibold"
                      />
                    )}
                  </button>
                  <Tooltip>
                    <TooltipTrigger
                      render={
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="me-[14px] flex-none text-ink-3 hover:text-negative"
                          aria-label={`Delete the valuation from ${stamp(valuation)}`}
                          onClick={() => {
                            setDialog({ kind: "delete", valuation })
                          }}
                        />
                      }
                    >
                      <Trash2 aria-hidden />
                    </TooltipTrigger>
                    <TooltipContent>Delete valuation</TooltipContent>
                  </Tooltip>
                </li>
              ))}
            </ul>
          </Panel>
          <Footnote>
            Values are in {currency ?? "the property's currency"}, as you
            entered them. The chart and every total use the latest valuation on
            or before each date.
          </Footnote>
        </div>
      )}

      {currency !== null &&
      (dialog?.kind === "add" || dialog?.kind === "edit") ? (
        <ValuationDialog
          property={property}
          currency={currency}
          valuation={dialog.kind === "edit" ? dialog.valuation : null}
          onOpenChange={closeDialog}
        />
      ) : null}

      <ConfirmDestructive
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !remove.isPending) setDialog(null)
        }}
        title="Delete valuation?"
        lost={
          deleting === null
            ? ""
            : `This removes the ${money(deleting.value)} valuation from ${stamp(deleting)}.`
        }
        {...(valuations.length === 1
          ? {
              survives:
                "The property will have no value until you add a new one.",
            }
          : {})}
        confirmLabel="Delete"
        cancelLabel="Cancel"
        pending={remove.isPending}
        onConfirm={() => {
          if (deleting === null) return
          remove.mutate(
            {
              assetId: property.assetId,
              currencyAssetId: property.currencyAssetId,
              date: deleting.date,
            },
            {
              onSettled: () => {
                setDialog(null)
              },
              onSuccess: () => {
                toast.add({
                  type: "success",
                  title: "Valuation deleted",
                  description: `The ${money(deleting.value)} valuation from ${stamp(deleting)} is gone.`,
                })
              },
            }
          )
        }}
      />
    </>
  )
}

export function PropertyValuations({ propertyId }: { propertyId: string }) {
  const properties = usePropertiesSuspense(useUserId())
  const property = properties.byId.get(propertyId)
  if (property === undefined) return <PropertyNotFound />
  return <ValuationsScreen key={property.propertyId} property={property} />
}
