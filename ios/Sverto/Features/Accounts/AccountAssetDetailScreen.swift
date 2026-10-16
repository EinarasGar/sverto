import SvertoCore
import SwiftUI

struct AccountAssetDetailScreen: View {
    let accountId: String
    let accountName: String
    let assetId: Int32

    @Environment(AppServices.self) private var services

    var body: some View {
        AccountAssetDetailContent(store: services.store, accountId: accountId, accountName: accountName, assetId: assetId)
    }
}

private struct AccountAssetDetailContent: View {
    @State private var model: AccountAssetDetailViewModel
    @State private var chartPeriod = ChartPeriod.defaultSelection
    @State private var isRevealed = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(ShellState.self) private var shell
    private let accountId: String
    private let accountName: String
    private let assetId: Int32

    init(store: AppStore, accountId: String, accountName: String, assetId: Int32) {
        self.accountId = accountId
        self.accountName = accountName
        self.assetId = assetId
        _model = State(initialValue: AccountAssetDetailViewModel(store: store, accountId: accountId, assetId: assetId))
    }

    private var currencySelection: Binding<Bool> {
        Binding(
            get: { model.showsBaseCurrency },
            set: { showBase in Task { await model.selectCurrency(showBase: showBase) } }
        )
    }

    var body: some View {
        content
            .navigationTitle(model.state.ticker)
            .navigationSubtitle(model.state.name)
            .navigationBarTitleDisplayMode(.inline)
            .task { await model.start() }
            .onDisappear { model.stop() }
    }

    @ViewBuilder
    private var content: some View {
        if model.state.isLoading {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.screenBackground)
        } else if let error = model.state.error {
            ContentUnavailableView {
                Label("Couldn't load holding", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            }
            .background(.screenBackground)
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if !model.state.chartData.isEmpty {
                        chartBlock
                            .modifier(StaggeredReveal(isRevealed: isRevealed, index: 0, reduceMotion: reduceMotion))
                    }
                    positionBlock
                        .modifier(StaggeredReveal(isRevealed: isRevealed, index: 1, reduceMotion: reduceMotion))
                    if !model.state.lots.isEmpty {
                        lotsBlock
                            .modifier(StaggeredReveal(isRevealed: isRevealed, index: 2, reduceMotion: reduceMotion))
                    }
                    Button {
                        let title = [model.state.ticker, model.state.name].first { !$0.isEmpty } ?? "Transactions"
                        shell.push(.assetTransactions(assetId: assetId, accountId: accountId, title: title, subtitle: accountName))
                    } label: {
                        Label("View transactions", systemImage: "list.bullet.rectangle.portrait")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .modifier(StaggeredReveal(isRevealed: isRevealed, index: 3, reduceMotion: reduceMotion))
                }
                .padding(.horizontal, Layout.cardPadding)
                .padding(.vertical, 8)
            }
            .groupedScreenBackground()
            .onAppear { isRevealed = true }
        }
    }

    private var chartBlock: some View {
        VStack(alignment: .leading, spacing: 12) {
            if model.state.supportsBaseConversion {
                Text("Chart currency")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Picker("Chart currency", selection: currencySelection) {
                    Text(model.state.priceTicker).tag(false)
                    Text(model.state.baseTicker).tag(true)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }
            if model.showsBaseCurrency, model.isBaseChartLoading {
                ChartSkeleton()
            } else {
                ChartView(periods: model.activeChartData, baseTicker: model.activeTicker, selectedPeriod: $chartPeriod)
            }
        }
    }

    private var positionBlock: some View {
        let state = model.state
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("My Position · \(state.baseTicker)")
            Text(Money.format(state.value, ticker: state.baseTicker))
                .font(.largeTitle.bold())
                .monospacedDigit()
                .accessibilityLabel("Position value \(Money.format(state.value, ticker: state.baseTicker))")
            MetricGrid(items: [
                MetricItem(label: "Cost Basis", value: Money.format(state.costBasis, ticker: state.baseTicker)),
                MetricItem(
                    label: "Unrealized P&L",
                    value: Money.format(state.unrealizedGains, ticker: state.baseTicker, signed: true),
                    valueColor: state.unrealizedGains >= 0 ? .gain : .loss
                ),
                MetricItem(label: "Units Held", value: AssetFormat.units(state.units)),
            ])
        }
    }

    private var lotsBlock: some View {
        let lots = model.state.lots
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Positions (\(lots.count) lots)")
            ForEach(Array(lots.enumerated()), id: \.offset) { _, lot in
                LotCard(lot: lot, baseTicker: model.state.baseTicker)
            }
        }
    }
}
