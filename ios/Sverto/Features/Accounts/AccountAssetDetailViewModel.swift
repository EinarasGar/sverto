import Foundation
import Observation
import SvertoCore

@Observable
final class AccountAssetDetailViewModel {
    private(set) var state = AssetDetailState(
        isLoading: true,
        error: nil,
        assetId: 0,
        ticker: "",
        name: "",
        units: 0,
        value: 0,
        costBasis: 0,
        unrealizedGains: 0,
        totalFees: 0,
        currentPrice: 0,
        chartData: [],
        lots: [],
        baseTicker: "",
        priceTicker: "",
        priceCurrencyId: 0,
        supportsBaseConversion: false,
        baseChartData: []
    )
    private(set) var showsBaseCurrency = false
    private(set) var isBaseChartLoading = false
    private(set) var selectedPeriod = ChartPeriod.defaultSelection
    private(set) var holdingCharts: [HoldingChartSeries: [ChartPeriod: [ChartPoint]]] = [:]
    private(set) var holdingChartErrors: [HoldingChartSeries: [ChartPeriod: String]] = [:]

    private let store: AppStore
    private let accountId: String
    private let assetId: Int32
    private var bridge: AssetDetailObserverBridge?
    private var hasLoaded = false
    private var chartGeneration = 0
    private var chosenSeries = HoldingChartSeries.value

    init(store: AppStore, accountId: String, assetId: Int32) {
        self.store = store
        self.accountId = accountId
        self.assetId = assetId
    }

    var isHeld: Bool { state.units != 0 }

    var chartSeries: HoldingChartSeries { isHeld ? chosenSeries : .price }

    var currencyTicker: String {
        showsBaseCurrency ? state.baseTicker : state.priceTicker
    }

    var chartTicker: String { chartSeries == .units ? "" : currencyTicker }

    var chartData: [ChartPeriodData] {
        switch chartSeries {
        case .price:
            return showsBaseCurrency ? state.baseChartData : state.chartData
        case .value, .units:
            let points = holdingCharts[chartSeries] ?? [:]
            return ChartPeriod.allCases.map { ChartPeriodData(period: $0.rawValue, points: points[$0] ?? []) }
        }
    }

    var isChartLoading: Bool {
        switch chartSeries {
        case .price: showsBaseCurrency && isBaseChartLoading
        case .value, .units: holdingCharts[chartSeries]?[selectedPeriod] == nil && chartError == nil
        }
    }

    var chartError: String? { holdingChartErrors[chartSeries]?[selectedPeriod] }

    func start() async {
        observeIfNeeded()
        guard !hasLoaded else { return }
        hasLoaded = true
        await store.loadAssetDetail(accountId: accountId, assetId: assetId)
    }

    func stop() {
        guard bridge != nil else { return }
        bridge = nil
        store.unobserveAssetDetail()
    }

    func selectCurrency(showBase: Bool) {
        guard showBase != showsBaseCurrency else { return }
        showsBaseCurrency = showBase
        chartGeneration += 1
        holdingCharts = [:]
        holdingChartErrors = [:]
        loadChartIfMissing()
    }

    func selectPeriod(_ period: ChartPeriod) {
        selectedPeriod = period
        loadChartIfMissing()
    }

    func selectSeries(_ series: HoldingChartSeries) {
        chosenSeries = series
        loadChartIfMissing()
    }

    func retryChart() async {
        await loadHoldingChart(chartSeries, selectedPeriod)
    }

    private func observeIfNeeded() {
        guard bridge == nil else { return }
        let bridge = AssetDetailObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.handleState(state) }
        }
        self.bridge = bridge
        store.observeAssetDetail(observer: bridge)
    }

    private func handleState(_ newState: AssetDetailState) {
        let previousSeries = chartSeries
        state = newState
        if !newState.isLoading, chartSeries != previousSeries {
            loadChartIfMissing()
        }
    }

    private func loadChartIfMissing() {
        let series = chartSeries
        let period = selectedPeriod
        switch series {
        case .price:
            guard showsBaseCurrency, state.baseChartData.isEmpty, !isBaseChartLoading else { return }
            isBaseChartLoading = true
            Task {
                await store.loadAssetDetailBaseChart()
                isBaseChartLoading = false
            }
        case .value, .units:
            guard holdingCharts[series]?[period] == nil else { return }
            Task { await loadHoldingChart(series, period) }
        }
    }

    private func loadHoldingChart(_ series: HoldingChartSeries, _ period: ChartPeriod) async {
        let generation = chartGeneration
        let range = period.apiRange
        holdingChartErrors[series]?[period] = nil
        do {
            let points: [ChartPoint]
            switch series {
            case .price:
                return
            case .value:
                guard let currencyId = showsBaseCurrency ? store.getDefaultAssetId() : state.priceCurrencyId else { return }
                points = try await store.getHoldingValueHistory(assetId: assetId, accountId: accountId, currencyId: currencyId, range: range)
            case .units:
                points = try await store.getHoldingUnitsHistory(assetId: assetId, accountId: accountId, range: range)
            }
            guard generation == chartGeneration else { return }
            holdingCharts[series, default: [:]][period] = points
        } catch {
            guard generation == chartGeneration else { return }
            holdingChartErrors[series, default: [:]][period] = error.friendlyMessage
        }
    }
}
