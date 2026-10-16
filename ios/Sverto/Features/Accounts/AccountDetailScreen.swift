import SvertoCore
import SwiftUI

struct AccountDetailScreen: View {
    let accountId: String
    let accountName: String
    let accountTypeId: Int32

    @Environment(AppServices.self) private var services

    var body: some View {
        AccountDetailContent(
            store: services.store,
            accountId: accountId,
            accountName: accountName,
            accountTypeId: accountTypeId
        )
    }
}

private struct AccountDetailContent: View {
    @State private var model: AccountDetailViewModel
    @State private var chartPeriod = ChartPeriod.defaultSelection
    @State private var isConfirmingDelete = false
    @State private var isRevealed = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    private let accountName: String

    init(store: AppStore, accountId: String, accountName: String, accountTypeId: Int32) {
        self.accountName = accountName
        _model = State(initialValue: AccountDetailViewModel(
            store: store,
            accountId: accountId,
            accountName: accountName,
            accountTypeId: accountTypeId
        ))
    }

    private var keyPrompt: Binding<AccountSyncPrompt?> {
        Binding(
            get: { model.syncState.prompt },
            set: { prompt in
                if prompt == nil {
                    model.dismissKeyPrompt()
                }
            }
        )
    }

    var body: some View {
        content
            .navigationTitle(accountName)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    menu
                        .tint(.primary)
                }
            }
            .confirmationDialog("Delete account", isPresented: $isConfirmingDelete, titleVisibility: .visible) {
                Button("Delete", role: .destructive) {
                    Task { await deleteAccount() }
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Are you sure you want to delete this account? This action cannot be undone.")
            }
            .sheet(item: keyPrompt) { prompt in
                TransientKeySheet(
                    onSave: { key in
                        Task { await model.saveTransientKey(connectionId: prompt.connectionId, key: key) }
                    },
                    onDismiss: { model.dismissKeyPrompt() }
                )
            }
            .onChange(of: model.syncState.message) { _, message in
                showSyncMessage(message)
            }
            .task {
                showSyncMessage(model.syncState.message)
                await model.start()
            }
            .onDisappear { model.stop() }
    }

    private var menu: some View {
        Menu {
            Button("Edit", systemImage: "pencil") {
                shell.push(.editAccount(accountId: model.accountId))
            }
            if model.syncState.canSync {
                Button(model.syncState.isSyncing ? "Syncing…" : "Sync", systemImage: "arrow.triangle.2.circlepath") {
                    Task { await model.sync() }
                }
                .disabled(model.syncState.isSyncing)
            }
            Button("Delete", systemImage: "trash", role: .destructive) {
                isConfirmingDelete = true
            }
        } label: {
            Image(systemName: "ellipsis")
        }
        .accessibilityLabel("More options")
    }

    @ViewBuilder
    private var content: some View {
        if model.showsLoading {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.screenBackground)
        } else if let error = model.state.error {
            ContentUnavailableView {
                Label("Couldn't load account", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            }
            .background(.screenBackground)
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if !model.state.chartData.isEmpty {
                        ChartView(periods: model.state.chartData, baseTicker: model.state.baseTicker, selectedPeriod: $chartPeriod)
                            .modifier(StaggeredReveal(isRevealed: isRevealed, index: 0, reduceMotion: reduceMotion))
                    }
                    if model.isInvestmentAccount {
                        investmentSummary
                            .modifier(StaggeredReveal(isRevealed: isRevealed, index: 1, reduceMotion: reduceMotion))
                        holdings
                            .modifier(StaggeredReveal(isRevealed: isRevealed, index: 2, reduceMotion: reduceMotion))
                    }
                    transactions
                        .modifier(StaggeredReveal(isRevealed: isRevealed, index: model.isInvestmentAccount ? 3 : 1, reduceMotion: reduceMotion))
                }
                .padding(.horizontal, Layout.cardPadding)
                .padding(.vertical, 8)
            }
            .groupedScreenBackground()
            .onAppear { isRevealed = true }
        }
    }

    private var investmentSummary: some View {
        let state = model.state
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Investment Summary")
            MetricGrid(items: [
                MetricItem(label: "Total Value", value: Money.format(state.totalValue, ticker: state.baseTicker)),
                MetricItem(label: "Cost Basis", value: Money.format(state.totalCostBasis, ticker: state.baseTicker)),
                MetricItem(
                    label: "Unrealized P&L",
                    value: Money.format(state.unrealizedGains, ticker: state.baseTicker, signed: true),
                    valueColor: state.unrealizedGains >= 0 ? .gain : .loss
                ),
                MetricItem(label: "Total Fees", value: Money.format(state.totalFees, ticker: state.baseTicker)),
            ])
        }
    }

    private var holdings: some View {
        let holdings = model.state.holdings
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Holdings", trailing: {
                AnyView(
                    Text("\(holdings.count) assets")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                )
            })
            GroupedCard {
                if holdings.isEmpty {
                    Text("No holdings in this account")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(Layout.cardPadding)
                } else {
                    VStack(spacing: 0) {
                        ForEach(Array(holdings.enumerated()), id: \.element.assetId) { index, holding in
                            HoldingRow(holding: holding, baseTicker: model.state.baseTicker) {
                                shell.push(.accountAssetDetail(accountId: model.accountId, accountName: accountName, assetId: holding.assetId))
                            }
                            if index < holdings.count - 1 {
                                RowDivider(leadingInset: Layout.cardPadding + 36 + Layout.rowSpacing)
                            }
                        }
                    }
                }
            }
        }
    }

    private var transactions: some View {
        let recent = model.state.recentTransactions
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Transactions", trailing: lastSyncedLabel)
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
                        RowDivider(leadingInset: Layout.cardPadding + 40 + Layout.rowSpacing)
                    }
                    Button("View more transactions") {
                        shell.push(.accountTransactions(accountId: model.accountId))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, Layout.rowSpacing)
                }
            }
        }
    }

    private var lastSyncedLabel: (() -> AnyView)? {
        guard let lastSyncAt = model.syncState.lastSyncAt else { return nil }
        return {
            AnyView(
                Text("Last synced \(RelativeTime.string(epochSeconds: lastSyncAt))")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            )
        }
    }

    private func showSyncMessage(_ message: String?) {
        guard let message else { return }
        toasts.show(message)
        model.consumeSyncMessage()
    }

    private func deleteAccount() async {
        if let error = await model.delete() {
            toasts.show(error)
            return
        }
        shell.pop()
    }
}
