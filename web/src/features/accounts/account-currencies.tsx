import { Link } from "@tanstack/react-router"

import { useBaseCurrency } from "@/auth"
import { Figure } from "@/components/figure"
import {
  focusRing,
  Panel,
  PanelFootnote,
  PanelHeader,
  PanelNote,
  PanelTitle,
  Truncate,
} from "@/components/primitives"
import type { Holding } from "@/features/portfolio/api"
import { useRequiredBaseAssetId } from "@/features/portfolio/api"
import type { AssetRef } from "@/lib/domain/refs"
import { assetLabel, isCurrencyAsset } from "@/lib/domain/refs"
import { cn } from "@/lib/utils"

import type { AccountDetail } from "./api"

type CurrencyHolding = Holding & { asset: AssetRef }

function isCurrencyHolding(holding: Holding): holding is CurrencyHolding {
  return holding.asset !== null && isCurrencyAsset(holding.asset)
}

export function AccountCurrencies({
  account,
  holdings,
}: {
  account: AccountDetail
  holdings: readonly Holding[]
}) {
  const baseAssetId = useRequiredBaseAssetId()
  const baseCurrency = useBaseCurrency()
  const isInvestment = account.accountClass === "investments"
  const rows = holdings
    .filter(isCurrencyHolding)
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0))

  if (rows.length < (isInvestment ? 1 : 2)) return null

  const title = isInvestment ? "Cash" : "Balances"

  return (
    <Panel data-slot="account-currencies">
      <PanelHeader>
        <PanelTitle>{title}</PanelTitle>
        <PanelNote>by currency</PanelNote>
      </PanelHeader>
      <ul className="list-none" aria-label={`${title} in this account`}>
        {rows.map((holding) => {
          const ticker = assetLabel(holding.asset)
          const name = holding.asset.name
          return (
            <li
              key={holding.assetId}
              data-slot="account-currency"
              className="relative flex h-[54px] items-center gap-[11px] border-b border-border px-[18px] last:border-b-0 focus-within:bg-surface-2 hover:bg-surface-2"
            >
              <div className="min-w-0 flex-1">
                <Link
                  to="/portfolio/$assetId"
                  params={{ assetId: String(holding.assetId) }}
                  search={{ account: account.accountId }}
                  className={cn(
                    "block min-w-0 text-[13px] leading-[1.3] font-medium outline-none",
                    "after:absolute after:inset-0 after:content-['']",
                    focusRing.row
                  )}
                >
                  <Truncate text={ticker} className="block" />
                </Link>
                {name === null || name === ticker ? null : (
                  <Truncate
                    text={name}
                    className="block text-[11px] leading-[1.4] text-ink-3"
                  />
                )}
              </div>
              <div className="flex flex-none flex-col items-end gap-[5px]">
                <Figure
                  value={holding.units}
                  currency={ticker}
                  size="base"
                  className="font-semibold"
                />
                {holding.assetId === baseAssetId ||
                holding.value === null ? null : (
                  <Figure
                    value={holding.value}
                    currency={baseCurrency}
                    intent="secondary"
                    size="micro"
                  />
                )}
              </div>
            </li>
          )
        })}
      </ul>
      {account.isJoint ? (
        <PanelFootnote>
          Amounts are your {String(Math.round(account.ownershipSharePercent))}%
          share, like the balance above.
        </PanelFootnote>
      ) : null}
    </Panel>
  )
}
