import SwiftUI
import SvertoCore

struct PortfolioContent: View {
    let model: HomeViewModel
    let onSelectHolding: (HoldingItem) -> Void
    let onSelectLiability: (LiabilityItem) -> Void

    @State private var selectedPeriod = ChartPeriod.defaultSelection

    private enum Phase {
        case loading
        case failed(String)
        case loaded
    }

    private var isEmpty: Bool {
        model.state.holdings.isEmpty && model.state.liabilities.isEmpty
    }

    private var phase: Phase {
        let state = model.state
        if state.isLoading, isEmpty {
            return .loading
        }
        if let error = state.error, isEmpty {
            return .failed(error)
        }
        return .loaded
    }

    private var chartPeriods: [ChartPeriodData] {
        model.state.chartData.filter { ChartPeriod(rawValue: $0.period) != nil }
    }

    var body: some View {
        List {
            switch phase {
            case .loading:
                PortfolioSkeleton()
                    .plainPortfolioRow()
            case let .failed(message):
                PortfolioErrorState(message: message) {
                    Task { await model.load() }
                }
                .plainPortfolioRow()
            case .loaded:
                if !chartPeriods.isEmpty {
                    ChartView(periods: chartPeriods, baseTicker: model.state.baseTicker, selectedPeriod: $selectedPeriod, plotOutset: Layout.cardPadding)
                        .plainPortfolioRow()
                }
                if isEmpty {
                    ContentUnavailableView(
                        "No holdings yet",
                        systemImage: "chart.pie",
                        description: Text("Add accounts and transactions to see your holdings here.")
                    )
                    .plainPortfolioRow()
                } else {
                    HoldingsList(state: model.state, onSelectHolding: onSelectHolding, onSelectLiability: onSelectLiability)
                }
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await model.refresh() }
    }
}

private extension View {
    func plainPortfolioRow() -> some View {
        listRowBackground(Color.clear)
            .listRowSeparator(.hidden)
            .listRowInsets(EdgeInsets(top: 8, leading: Layout.cardPadding, bottom: 8, trailing: Layout.cardPadding))
    }
}
