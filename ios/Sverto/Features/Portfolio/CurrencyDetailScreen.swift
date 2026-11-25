import SvertoCore
import SwiftUI

struct CurrencyDetailScreen: View {
    let assetId: Int32
    let name: String
    let accountId: String?

    @Environment(AppServices.self) private var services

    var body: some View {
        CurrencyDetailContent(store: services.store, assetId: assetId, name: name, accountId: accountId)
    }
}

private struct CurrencyDetailContent: View {
    @State private var model: CurrencyDetailViewModel
    @State private var chartPeriod = ChartPeriod.defaultSelection
    @Environment(ShellState.self) private var shell

    private let assetId: Int32
    private let name: String
    private let accountId: String?

    init(store: AppStore, assetId: Int32, name: String, accountId: String?) {
        self.assetId = assetId
        self.name = name
        self.accountId = accountId
        _model = State(initialValue: CurrencyDetailViewModel(store: store, assetId: assetId, accountId: accountId))
    }

    private var accountName: String? {
        accountId == nil ? nil : model.detail?.accounts.first?.accountName
    }

    var body: some View {
        content
            .navigationTitle(model.detail?.name ?? name)
            .navigationSubtitle(accountName.map { "in \($0)" } ?? "")
            .navigationBarTitleDisplayMode(.inline)
            .task { await model.load() }
    }

    @ViewBuilder
    private var content: some View {
        if let detail = model.detail {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    header(detail)
                    if !detail.chartData.isEmpty {
                        ChartView(
                            periods: detail.chartData,
                            baseTicker: detail.ticker,
                            selectedPeriod: $chartPeriod,
                            showHeadline: false
                        )
                    }
                    if accountId == nil {
                        accounts(detail)
                    }
                    transactions(detail)
                }
                .padding(.horizontal, Layout.cardPadding)
                .padding(.vertical, 8)
            }
            .groupedScreenBackground()
            .refreshable { await model.refresh() }
        } else if let error = model.loadError {
            ContentUnavailableView {
                Label("Couldn't load currency", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") {
                    Task { await model.load() }
                }
                .buttonStyle(.borderedProminent)
            }
            .background(.screenBackground)
        } else {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.screenBackground)
        }
    }

    private func header(_ detail: CurrencyDetail) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(Money.format(detail.units, ticker: detail.ticker))
                .font(.largeTitle.weight(.semibold))
                .monospacedDigit()
            if !detail.isBaseCurrency, let value = detail.value {
                Text(Money.format(value, ticker: detail.baseTicker))
                    .font(.subheadline)
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
            }
            if let rate = detail.rate {
                Button {
                    shell.push(.assetOverview(assetId: assetId, userAsset: false))
                } label: {
                    HStack(spacing: 4) {
                        Text("1 \(detail.ticker) = \(AssetFormat.rate(rate)) \(detail.baseTicker)")
                            .monospacedDigit()
                        Image(systemName: "chevron.right")
                            .font(.caption.weight(.semibold))
                            .accessibilityHidden(true)
                    }
                    .font(.subheadline)
                }
                .padding(.top, 4)
                .accessibilityHint("Opens the exchange rate")
            }
        }
    }

    private func accounts(_ detail: CurrencyDetail) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Accounts")
            GroupedCard {
                if detail.accounts.isEmpty {
                    Text("No accounts hold \(detail.ticker)")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(Layout.cardPadding)
                } else {
                    VStack(spacing: 0) {
                        ForEach(Array(detail.accounts.enumerated()), id: \.element.accountId) { index, account in
                            Button {
                                shell.push(.accountDetail(
                                    accountId: account.accountId,
                                    accountName: account.accountName,
                                    accountTypeId: account.accountTypeId
                                ))
                            } label: {
                                CurrencyAccountRow(account: account, ticker: detail.ticker)
                            }
                            .buttonStyle(.plain)
                            if index < detail.accounts.count - 1 {
                                RowDivider(leadingInset: Layout.cardPadding + 36 + Layout.rowSpacing)
                            }
                        }
                    }
                }
            }
        }
    }

    private func transactions(_ detail: CurrencyDetail) -> some View {
        let recent = detail.recentTransactions
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Recent transactions")
            GroupedCard {
                VStack(spacing: 0) {
                    if recent.isEmpty {
                        Text("No transactions yet")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity)
                            .padding(Layout.cardPadding)
                    }
                    ForEach(recent, id: \.id) { transaction in
                        Button {
                            shell.showTransactionDetail(transaction, isInGroup: false)
                        } label: {
                            AccountTransactionRow(transaction: transaction)
                                .padding(.horizontal, Layout.cardPadding)
                                .padding(.vertical, Layout.rowSpacing)
                                .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        if detail.hasMoreTransactions || transaction.id != recent.last?.id {
                            RowDivider(leadingInset: Layout.cardPadding + 40 + Layout.rowSpacing)
                        }
                    }
                    if detail.hasMoreTransactions {
                        Button("View all") {
                            if let accountId {
                                shell.push(.assetTransactions(assetId: assetId, accountId: accountId, title: detail.ticker, subtitle: accountName))
                            } else {
                                shell.push(.currencyTransactions(assetId: assetId, title: detail.ticker))
                            }
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, Layout.rowSpacing)
                    }
                }
            }
        }
    }
}

private struct CurrencyAccountRow: View {
    let account: CurrencyAccount
    let ticker: String

    var body: some View {
        HStack(spacing: Layout.rowSpacing) {
            AccountTypeBadge(typeId: account.accountTypeId, size: 36)
            VStack(alignment: .leading, spacing: 2) {
                Text(account.accountName)
                    .font(.body.weight(.medium))
                    .fixedSize(horizontal: false, vertical: true)
                Text(AccountTypeVisuals.label(account.accountTypeId))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 8)
            Text(Money.format(account.units, ticker: ticker))
                .font(.body.weight(.semibold))
                .monospacedDigit()
                .lineLimit(1)
                .fixedSize()
        }
        .padding(.vertical, Layout.rowSpacing)
        .padding(.horizontal, Layout.cardPadding)
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
    }
}
