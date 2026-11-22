import { useState } from "react"
import { Link } from "@tanstack/react-router"
import { History, PencilIcon, Plus } from "lucide-react"

import { useUserId } from "@/auth"
import { Figure } from "@/components/figure"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import {
  CountChip,
  Footnote,
  PageHeader,
  PageHeaderBackLink,
  SectionHeader,
} from "@/components/primitives"
import { Button, buttonVariants } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { AccountRecentTransactions } from "@/features/accounts/account-recent"
import { Tile } from "@/features/accounts/account-tiles"
import {
  useAccountBalancesSuspense,
  useAccountsSuspense,
} from "@/features/accounts/api"
import { AccountPanelSkeleton } from "@/features/accounts/skeletons"
import {
  useHoldingsSuspense,
  useRequiredBaseAssetId,
} from "@/features/portfolio/api"
import { countOf } from "@/lib/format"
import { cn } from "@/lib/utils"

import type { Property } from "./api"
import { usePropertiesSuspense } from "./api"
import { EditPropertyDialog } from "./edit-property-dialog"
import { propertyFigures } from "./figures"
import { LinkLoanDialog } from "./link-loan-dialog"
import { PROPERTY_KIND_ICONS, PROPERTY_KIND_LABELS } from "./presentation"
import { PropertyLoans } from "./property-loans"
import { PropertyNotFound } from "./property-not-found"
import { PropertyValueChart } from "./property-value-chart"
import { usePropertyCurrency } from "./use-property-currency"
import { ValuationDialog } from "./valuation-dialog"

type PropertyDialog = "value" | "edit" | "link"

function PropertyBody({
  property,
  onLinkLoan,
}: {
  property: Property
  onLinkLoan: () => void
}) {
  const userId = useUserId()
  const defaultAssetId = useRequiredBaseAssetId()
  const accounts = useAccountsSuspense(userId)
  const wholeHoldings = useHoldingsSuspense({
    userId,
    defaultAssetId,
    applyOwnershipShare: false,
  })
  const balances = useAccountBalancesSuspense({ userId, defaultAssetId })
  const figures = propertyFigures(property, accounts, wholeHoldings)
  const yourEquity =
    balances.accounts.find((row) => row.accountId === property.accountId)
      ?.value ?? null
  const loanCount = figures.loans.length

  return (
    <div className="flex flex-col gap-[26px]">
      <PropertyValueChart property={property} currentValue={figures.value} />
      <div>
        <div
          data-slot="property-tiles"
          className={cn(
            "grid grid-cols-2 gap-px overflow-hidden rounded-panel border border-border bg-border",
            property.isJoint ? "lg:grid-cols-5" : "lg:grid-cols-4"
          )}
        >
          <Tile label="Value">
            <Figure value={figures.value} size="md" />
          </Tile>
          <Tile
            label="Loan left"
            note={loanCount === 0 ? "none linked" : countOf(loanCount, "loan")}
          >
            <Figure value={figures.loanBalance} size="md" />
          </Tile>
          <Tile label="Equity">
            <Figure value={figures.equity} size="md" />
          </Tile>
          <Tile label="Loan-to-value">
            <Figure
              value={figures.loanToValue}
              kind="percent"
              scale="ratio"
              size="md"
            />
          </Tile>
          {property.isJoint ? (
            <Tile
              label="Your share"
              note="of equity"
              className="col-span-2 lg:col-span-1"
            >
              <Figure value={yourEquity} size="md" />
            </Tile>
          ) : null}
        </div>
        <Footnote className="mt-[10px]">
          Value, loan left and equity are for the whole{" "}
          {property.kind === "vehicle" ? "vehicle" : "property"} and its loans,
          in your base currency at today&rsquo;s rates.
          {property.isJoint
            ? " Your share applies each account's ownership share, the same figure your accounts list and net worth use."
            : ""}
        </Footnote>
      </div>

      <PropertyLoans
        property={property}
        loans={figures.loans}
        onLinkLoan={onLinkLoan}
      />

      {figures.loans.map((loan) => (
        <section key={loan.accountId} data-slot="property-loan-activity">
          <SectionHeader
            label={loan.account?.name ?? "Loan"}
            note="loan activity"
          />
          <AccountRecentTransactions
            accountId={loan.accountId}
            accountName={loan.account?.name ?? "this loan"}
          />
        </section>
      ))}
    </div>
  )
}

function PropertyScreen({ property }: { property: Property }) {
  const currency = usePropertyCurrency(property)
  const [dialog, setDialog] = useState<PropertyDialog | null>(null)
  const KindIcon = PROPERTY_KIND_ICONS[property.kind]

  const closeDialog = (open: boolean) => {
    if (!open) setDialog(null)
  }

  return (
    <>
      <PageHeader
        back={
          <PageHeaderBackLink render={<Link to="/accounts" />}>
            Accounts
          </PageHeaderBackLink>
        }
        eyebrow={
          <span className="inline-flex items-center gap-[6px]">
            <KindIcon aria-hidden className="size-3" />
            {PROPERTY_KIND_LABELS[property.kind]}
          </span>
        }
        title={property.name}
        meta={
          property.isJoint ? (
            <CountChip>
              Joint · {Math.round(property.ownershipSharePercent)}% yours
            </CountChip>
          ) : null
        }
        actions={
          <>
            <Button
              variant="outline"
              size="lg"
              onClick={() => {
                setDialog("edit")
              }}
            >
              <PencilIcon aria-hidden data-icon="inline-start" />
              Edit
            </Button>
            <div className="flex items-center">
              <Button
                size="lg"
                className="rounded-r-none"
                disabled={currency === null}
                onClick={() => {
                  setDialog("value")
                }}
              >
                <Plus aria-hidden data-icon="inline-start" />
                Add valuation
              </Button>
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Link
                      to="/properties/$propertyId/valuations"
                      params={{ propertyId: property.propertyId }}
                      aria-label="Valuations"
                      className={cn(
                        buttonVariants({ size: "lg" }),
                        "rounded-l-none border-l border-l-scrim px-2"
                      )}
                    />
                  }
                >
                  <History aria-hidden className="size-3.5" />
                </TooltipTrigger>
                <TooltipContent>Valuations</TooltipContent>
              </Tooltip>
            </div>
          </>
        }
      />
      <div className="flex flex-col gap-[26px]">
        <PanelBoundary
          pending={<AccountPanelSkeleton label="Loading property" rows={4} />}
        >
          <PropertyBody
            property={property}
            onLinkLoan={() => {
              setDialog("link")
            }}
          />
        </PanelBoundary>
      </div>

      {dialog === "value" && currency !== null ? (
        <ValuationDialog
          property={property}
          currency={currency}
          onOpenChange={closeDialog}
        />
      ) : null}
      {dialog === "edit" ? (
        <EditPropertyDialog property={property} onOpenChange={closeDialog} />
      ) : null}
      {dialog === "link" ? (
        <LinkLoanDialog property={property} onOpenChange={closeDialog} />
      ) : null}
    </>
  )
}

export function PropertyDetail({ propertyId }: { propertyId: string }) {
  const properties = usePropertiesSuspense(useUserId())
  const property = properties.byId.get(propertyId)

  if (property === undefined) return <PropertyNotFound />

  return <PropertyScreen key={property.propertyId} property={property} />
}
