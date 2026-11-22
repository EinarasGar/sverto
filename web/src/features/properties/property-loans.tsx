import { useState } from "react"
import { Link } from "@tanstack/react-router"
import { Plus, Unlink } from "lucide-react"

import { useUserId } from "@/auth"
import { Figure } from "@/components/figure"
import {
  CountChip,
  EntityMark,
  Panel,
  PanelBody,
  PanelHeader,
  PanelNote,
  PanelTitle,
  ROW_ACTIVATION_CLASS,
  Truncate,
} from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { toast } from "@/components/ui/toast"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import type { AccountConnector } from "@/features/accounts/api"
import { useAccountConnectors } from "@/features/accounts/api"
import { ConnectorStatus } from "@/features/accounts/connector-status"
import { ConfirmDestructive } from "@/features/settings/confirm-dialog"
import { countOf } from "@/lib/format"
import { cn } from "@/lib/utils"

import type { Property } from "./api"
import { useLinkPropertyLoan, useUnlinkPropertyLoan } from "./api"
import type { PropertyLoan } from "./figures"

function loanName(loan: PropertyLoan): string {
  return loan.account?.name ?? "Unknown account"
}

function LoanRow({
  loan,
  connector,
  onUnlink,
}: {
  loan: PropertyLoan
  connector: AccountConnector | null
  onUnlink: () => void
}) {
  const name = loanName(loan)
  const meta = [
    loan.account?.accountTypeName ?? "Loan",
    ...(loan.account?.isJoint
      ? [
          `your ${String(Math.round(loan.account.ownershipSharePercent))}% share`,
        ]
      : []),
  ].join(" · ")

  return (
    <li
      data-slot="property-loan-row"
      className="flex items-center border-b border-border last:border-b-0"
    >
      <Link
        to="/accounts/$accountId"
        params={{ accountId: loan.accountId }}
        className={cn(
          "flex min-h-[58px] min-w-0 flex-1 items-center gap-3 py-[10px] ps-[18px] pe-3",
          ROW_ACTIVATION_CLASS
        )}
      >
        <EntityMark seed={loan.accountId} label={name} size="md" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <Truncate
              text={name}
              className="min-w-0 text-[13.5px] leading-[1.3] font-medium"
            />
            {loan.account?.isJoint ? (
              <CountChip aria-hidden>
                {Math.round(loan.account.ownershipSharePercent)}%
              </CountChip>
            ) : null}
          </div>
          <Truncate
            text={meta}
            className="block text-[11px] leading-[1.5] text-ink-3"
          />
          {connector === null ? null : (
            <ConnectorStatus connector={connector} className="mt-[3px]" />
          )}
        </div>
        <Figure
          value={loan.balance}
          size="base"
          className="flex-none text-[14px] font-semibold"
        />
      </Link>
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              className="me-[14px] flex-none text-ink-3 hover:text-negative"
              aria-label={`Unlink ${name}`}
              onClick={onUnlink}
            />
          }
        >
          <Unlink aria-hidden />
        </TooltipTrigger>
        <TooltipContent>Unlink {name}</TooltipContent>
      </Tooltip>
    </li>
  )
}

export function PropertyLoans({
  property,
  loans,
  onLinkLoan,
}: {
  property: Property
  loans: readonly PropertyLoan[]
  onLinkLoan: () => void
}) {
  const userId = useUserId()
  const unlink = useUnlinkPropertyLoan(userId)
  const link = useLinkPropertyLoan(userId)
  const connectors = useAccountConnectors(userId).data
  const [unlinking, setUnlinking] = useState<PropertyLoan | null>(null)

  function unlinkLoan(loan: PropertyLoan) {
    const variables = {
      propertyId: property.propertyId,
      accountId: loan.accountId,
    }
    const name = loanName(loan)
    unlink.mutate(variables, {
      onSettled: () => {
        setUnlinking(null)
      },
      onSuccess: () => {
        toast.add({
          type: "success",
          title: "Loan unlinked",
          description: `${name} is back on your accounts list and no longer counts against ${property.name}.`,
          actionProps: {
            children: "Undo",
            onClick: () => {
              link.mutate(variables)
            },
          },
        })
      },
    })
  }

  return (
    <Panel data-slot="property-loans">
      <PanelHeader>
        <PanelTitle>Loans</PanelTitle>
        <PanelNote>{countOf(loans.length, "loan")} linked</PanelNote>
        <Button variant="outline" size="xs" onClick={onLinkLoan}>
          <Plus aria-hidden className="size-3" />
          Link loan
        </Button>
      </PanelHeader>
      {loans.length === 0 ? (
        <PanelBody className="text-[12px] leading-[1.5] text-ink-3">
          No loan is linked, so its equity is its whole value. Link a mortgage
          or loan account and it moves here from your accounts list.
        </PanelBody>
      ) : (
        <ul className="list-none">
          {loans.map((loan) => (
            <LoanRow
              key={loan.accountId}
              loan={loan}
              connector={connectors?.byAccountId[loan.accountId] ?? null}
              onUnlink={() => {
                setUnlinking(loan)
              }}
            />
          ))}
        </ul>
      )}
      <ConfirmDestructive
        open={unlinking !== null}
        onOpenChange={(open) => {
          if (!open && !unlink.isPending) setUnlinking(null)
        }}
        title="Unlink loan?"
        lost={`Are you sure you want to unlink ${unlinking === null ? "this loan" : loanName(unlinking)} from ${property.name}?`}
        survives="The loan stays a normal account and shows in your accounts list again."
        confirmLabel="Unlink"
        cancelLabel="Cancel"
        pending={unlink.isPending}
        onConfirm={() => {
          if (unlinking !== null) unlinkLoan(unlinking)
        }}
      />
    </Panel>
  )
}
