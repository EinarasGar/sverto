import { startTransition, useMemo, type ReactNode } from "react"
import { useQuery } from "@tanstack/react-query"
import { Link } from "@tanstack/react-router"

import { useBaseCurrency, useUserId } from "@/auth"
import type { ChartPeriod } from "@/components/chart"
import { HeroChart, HeroChartSkeleton } from "@/components/chart"
import { Figure } from "@/components/figure"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import { usePeriodSearch } from "@/components/layout/search-state"
import {
  EntityMark,
  focusRing,
  HIT_TARGET_ROW,
  PageHeader,
  PageHeaderBackLink,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  Truncate,
} from "@/components/primitives"
import { buttonVariants } from "@/components/ui/button"
import { RecentTransactions } from "@/features/accounts/account-recent"
import type { HistorySeries, HoldingsView } from "@/features/portfolio/api"
import {
  foldHoldings,
  historyChartPoints,
  useAccountPortfolioHistorySuspense,
  usePortfolioHistorySuspense,
  useRequiredBaseAssetId,
} from "@/features/portfolio/api"
import { propertiesQueryOptions } from "@/features/properties/api"
import type { LedgerFilterToken } from "@/features/transactions/api"
import type { AssetRef } from "@/lib/domain/refs"
import { accountLabel } from "@/lib/domain/refs"
import { cn } from "@/lib/utils"

interface CurrencyChartProps {
  assetId: number
  ticker: string
  units: number
  summary: ReactNode
  period: ChartPeriod
  onPeriodChange: (next: ChartPeriod) => void
}

function CurrencyHeroChart({
  series,
  ticker,
  units,
  summary,
  period,
  onPeriodChange,
}: CurrencyChartProps & { series: HistorySeries }) {
  return (
    <HeroChart
      data={historyChartPoints(series)}
      value={units}
      currency={ticker}
      label={`${ticker} held`}
      size="tall"
      period={period}
      onPeriodChange={onPeriodChange}
      summary={summary}
      emptyLabel={`The ${ticker} you hold is drawn here once there are a few days of history`}
    />
  )
}

function PortfolioCurrencyChart(props: CurrencyChartProps) {
  const series = usePortfolioHistorySuspense({
    userId: useUserId(),
    defaultAssetId: props.assetId,
    range: props.period,
    assetId: props.assetId,
    excludeLiabilities: true,
  })
  return <CurrencyHeroChart series={series} {...props} />
}

function AccountCurrencyChart({
  accountId,
  ...props
}: CurrencyChartProps & { accountId: string }) {
  const series = useAccountPortfolioHistorySuspense({
    userId: useUserId(),
    accountId,
    defaultAssetId: props.assetId,
    range: props.period,
    assetId: props.assetId,
  })
  return <CurrencyHeroChart series={series} {...props} />
}

export function CurrencyScreen({
  asset,
  holdings,
  accountId,
}: {
  asset: AssetRef
  holdings: HoldingsView
  accountId?: string | undefined
}) {
  const userId = useUserId()
  const defaultAssetId = useRequiredBaseAssetId()
  const baseCurrency = useBaseCurrency()
  const { data: properties } = useQuery(propertiesQueryOptions(userId))
  const [period, setPeriod] = usePeriodSearch()

  const assetId = asset.assetId
  const ticker = asset.ticker ?? String(assetId)
  const entry = useMemo(
    () => foldHoldings(holdings, properties).assetsById[assetId],
    [holdings, properties, assetId]
  )
  const accounts = useMemo(
    () => [...(entry?.holdings ?? [])].sort((a, b) => b.units - a.units),
    [entry]
  )
  const inAccount =
    accountId === undefined ? undefined : holdings.byAccountId[accountId]
  const accountName =
    inAccount?.account == null
      ? "Unknown account"
      : accountLabel(inAccount.account)
  const accountHolding = inAccount?.holdings.find(
    (holding) => holding.assetId === assetId
  )
  const tokens = useMemo(
    (): LedgerFilterToken[] =>
      accountId === undefined
        ? [
            {
              key: "asset",
              assetId,
              label: `${ticker} excl. liabilities`,
              excludeLiabilities: true,
            },
          ]
        : [
            { key: "account", accountId, label: accountName },
            { key: "asset", assetId, label: ticker },
          ],
    [assetId, ticker, accountId, accountName]
  )
  const transactionsSearch =
    accountId === undefined
      ? { asset: assetId, excludeLiabilities: true }
      : { account: accountId, asset: assetId }

  const units =
    accountId === undefined ? (entry?.units ?? 0) : (accountHolding?.units ?? 0)
  const value =
    accountId === undefined
      ? entry === undefined || entry.ratelessCount > 0
        ? null
        : entry.value
      : (accountHolding?.value ?? null)
  const isBase = assetId === defaultAssetId
  const rate = isBase || value === null || units === 0 ? null : value / units

  const changePeriod = (next: ChartPeriod) => {
    startTransition(() => {
      setPeriod(next)
    })
  }

  const viewAll = (
    <Link
      to="/transactions"
      search={transactionsSearch}
      className={cn("min-w-0 outline-none", HIT_TARGET_ROW, focusRing.chip)}
    >
      <Truncate text={`View all in ${ticker} →`} className="block" />
    </Link>
  )

  const summary = isBase ? null : (
    <div className="flex flex-col gap-[6px] text-[12px] leading-none text-ink-2">
      <span>
        <Figure
          value={value}
          currency={baseCurrency}
          intent="secondary"
          size="base"
          emptyLabel={`No rate to ${baseCurrency}`}
        />{" "}
        in {baseCurrency}
      </span>
      {rate === null ? null : (
        <span data-slot="currency-rate">
          1 {ticker} ={" "}
          <Figure
            value={rate}
            kind="units"
            ticker={baseCurrency}
            decimals={4}
            intent="secondary"
            size="micro"
            className="text-[12px]"
          />
        </span>
      )}
    </div>
  )

  const chart: CurrencyChartProps = {
    assetId,
    ticker,
    units,
    summary,
    period,
    onPeriodChange: changePeriod,
  }

  return (
    <>
      <PageHeader
        back={
          accountId === undefined ? (
            <PageHeaderBackLink render={<Link to="/portfolio" />}>
              Portfolio
            </PageHeaderBackLink>
          ) : (
            <PageHeaderBackLink
              render={<Link to="/accounts/$accountId" params={{ accountId }} />}
            >
              {accountName}
            </PageHeaderBackLink>
          )
        }
        eyebrow={
          accountId === undefined ? "Currency" : `Currency in ${accountName}`
        }
        title={ticker}
        {...(asset.name === null || asset.name === ticker
          ? {}
          : { meta: asset.name })}
        actions={
          <Link
            to="/transactions"
            search={transactionsSearch}
            className={buttonVariants({ variant: "outline", size: "lg" })}
          >
            View transactions
          </Link>
        }
      />

      <div className="flex flex-col gap-[26px]">
        <PanelBoundary
          pending={
            <HeroChartSkeleton
              size="tall"
              label={`Loading the ${ticker} you held`}
              period={period}
              onPeriodChange={changePeriod}
            />
          }
        >
          {accountId === undefined ? (
            <PortfolioCurrencyChart {...chart} />
          ) : (
            <AccountCurrencyChart accountId={accountId} {...chart} />
          )}
        </PanelBoundary>

        {accountId !== undefined ? null : (
          <Panel>
            <PanelHeader>
              <PanelTitle>Accounts</PanelTitle>
            </PanelHeader>
            {accounts.length === 0 ? (
              <PanelBody className="text-[12px] leading-[1.5] text-ink-3">
                No account outside your credit cards, mortgages and loans holds{" "}
                {ticker}.
              </PanelBody>
            ) : (
              <ul
                className="list-none"
                aria-label={`Accounts holding ${ticker}`}
              >
                {accounts.map((holding) => {
                  const label =
                    holding.account === null
                      ? "Unknown account"
                      : accountLabel(holding.account)
                  return (
                    <li
                      key={holding.accountId}
                      data-slot="currency-account"
                      className="relative flex h-[52px] items-center gap-[11px] border-b border-border px-[18px] last:border-b-0 focus-within:bg-surface-2 hover:bg-surface-2"
                    >
                      <EntityMark seed={holding.accountId} label={label} />
                      <Link
                        to="/accounts/$accountId"
                        params={{ accountId: holding.accountId }}
                        className={cn(
                          "min-w-0 flex-1 text-[13px] leading-[1.3] font-medium outline-none",
                          "after:absolute after:inset-0 after:content-['']",
                          focusRing.row
                        )}
                      >
                        <Truncate text={label} className="block" />
                      </Link>
                      <Figure
                        value={holding.units}
                        kind="units"
                        ticker={ticker}
                        size="base"
                        className="flex-none"
                      />
                    </li>
                  )
                })}
              </ul>
            )}
          </Panel>
        )}

        <RecentTransactions
          tokens={tokens}
          scope={`in ${ticker}`}
          emptyBody={`Nothing has moved ${ticker} in ${accountId === undefined ? "your accounts" : accountName} yet. Transactions show up here newest first.`}
          viewAll={viewAll}
          footnote={`${accountId === undefined ? `Credit cards, mortgages and loans are left out here, as they are from the ${ticker} amount above. ` : ""}Amounts are in the asset each entry moved, not converted to your base currency.`}
        />
      </div>
    </>
  )
}
