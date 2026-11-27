import SwiftUI
import SvertoCore

struct PortfolioContent: View {
    let model: HomeViewModel
    let onSelectHolding: (HoldingItem) -> Void
    let onSelectLiability: (LiabilityItem) -> Void

    @State private var selectedPeriod = ChartPeriod.defaultSelection
    @State private var width = CGFloat.infinity
    @State private var listScroll = RootScrollMemory()
    @State private var fold: CGRect?
    @Environment(\.horizontalSizeClass) private var sizeClass

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

    private var showsChartColumn: Bool {
        guard sizeClass == .regular, width >= Layout.minListDetailWidth, case .loaded = phase else { return false }
        return !chartPeriods.isEmpty
    }

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            if showsChartColumn {
                VStack {
                    ChartView(periods: chartPeriods, baseTicker: model.state.baseTicker, selectedPeriod: $selectedPeriod)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, Layout.cardPadding + 4)
                .padding(.vertical, 8)
                .containerRelativeFrame(.horizontal) { length, _ in fold?.minX ?? length / 2 }
                .frame(maxHeight: .infinity)
                .background(.screenBackground)
                if let fold {
                    Color.screenBackground
                        .frame(width: fold.width)
                        .ignoresSafeArea()
                }
            }
            // Recreated when the chart moves in or out; reusing the list there left the large title hidden after launch.
            list
                .id(showsChartColumn)
                .environment(\.rootScroll, listScroll)
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width.rounded() } action: { width = $0 }
        .onVerticalFoldChange { fold = $0 }
    }

    private var list: some View {
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
                if !chartPeriods.isEmpty, !showsChartColumn {
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
        .keepsRootScrollOffset()
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
