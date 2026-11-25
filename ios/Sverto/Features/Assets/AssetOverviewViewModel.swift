import Foundation
import Observation
import SvertoCore

@Observable
final class AssetOverviewViewModel {
    private(set) var isLoading = true
    private(set) var overview: AssetOverviewState?
    private(set) var detail: AssetDetail?
    private(set) var selectedPairId: Int32?
    private(set) var pairInfo: AssetPairDetail?
    private(set) var selectedPeriod = ChartPeriod.defaultSelection
    private(set) var chartByPeriod: [HoldingChartSeries: [ChartPeriod: [ChartPoint]]] = [:]
    private(set) var chartErrors: [HoldingChartSeries: [ChartPeriod: String]] = [:]
    private(set) var isHeld = false
    private(set) var isUserAsset = false
    private(set) var isReloading = false
    private(set) var lastError: AssetErrorEvent?

    private let store: AppStore
    private var assetId: Int32 = 0
    private var bridge: AssetOverviewObserverBridge?
    private var chartGeneration = 0
    private var isHoldingKnown = false
    private var chosenSeries = HoldingChartSeries.value

    init(store: AppStore) {
        self.store = store
    }

    var pairs: [AssetPairRef] { detail?.pairs ?? [] }

    var selectedPair: AssetPairRef? { pair(selectedPairId) }

    var title: String { detail?.displaySymbol ?? overview?.ticker ?? "" }

    var subtitle: String { detail?.name ?? overview?.name ?? "" }

    var referenceTicker: String { selectedPair?.ticker ?? overview?.priceTicker ?? "" }

    var chartSeries: HoldingChartSeries { isHeld ? chosenSeries : .price }

    var chartTicker: String { chartSeries == .units ? "" : referenceTicker }

    var isChartLoading: Bool {
        if isLoading { return true }
        guard selectedPairId != nil else { return false }
        return !isHoldingKnown || (chartByPeriod[chartSeries]?[selectedPeriod] == nil && chartError == nil)
    }

    var chartError: String? { chartErrors[chartSeries]?[selectedPeriod] }

    var chartPeriods: [ChartPeriodData] {
        let points = chartByPeriod[chartSeries] ?? [:]
        return ChartPeriod.allCases.map { ChartPeriodData(period: $0.rawValue, points: points[$0] ?? []) }
    }

    func start() {
        guard bridge == nil else { return }
        let bridge = AssetOverviewObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.handleOverview(state) }
        }
        self.bridge = bridge
        store.observeAssetOverview(observer: bridge)
    }

    func stop() {
        guard bridge != nil else { return }
        bridge = nil
        store.unobserveAssetOverview()
    }

    func load(assetId: Int32, userAsset: Bool) async {
        let alreadyLoaded = assetId == self.assetId && userAsset == isUserAsset && detail != nil
        if alreadyLoaded { return }
        self.assetId = assetId
        await reload(userAsset: userAsset)
    }

    func selectPair(_ referenceId: Int32) {
        chartGeneration += 1
        selectedPairId = referenceId
        pairInfo = nil
        chartByPeriod = [:]
        chartErrors = [:]
        isReloading = true
        Task { await loadPairInfo(referenceId) }
        if isHoldingKnown {
            loadChartIfMissing()
        }
        let store = self.store
        let assetId = self.assetId
        Task { await store.loadAssetOverview(assetId: assetId, referenceAssetId: referenceId) }
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
        await loadChart(chartSeries, selectedPeriod)
    }

    func addRate(referenceId: Int32, date: Int64, rate: Double) async throws {
        let targetAssetId = assetId
        try await store.addUserAssetRate(assetId: targetAssetId, referenceId: referenceId, date: date, rate: rate)
        guard assetId == targetAssetId, selectedPairId == referenceId else { return }
        selectPair(referenceId)
    }

    func addPair(_ referenceId: Int32) async {
        do {
            try await store.addUserAssetPair(assetId: assetId, referenceId: referenceId)
            await reload(userAsset: isUserAsset)
        } catch {
            report(error)
        }
    }

    func delete() async -> Bool {
        do {
            try await store.deleteUserAsset(assetId: assetId)
            return true
        } catch {
            report(error)
            return false
        }
    }

    private func reload(userAsset: Bool) async {
        isLoading = true
        overview = nil
        detail = nil
        selectedPairId = nil
        pairInfo = nil
        selectedPeriod = ChartPeriod.defaultSelection
        chartByPeriod = [:]
        chartErrors = [:]
        isHeld = false
        isHoldingKnown = false
        chosenSeries = .value
        isUserAsset = userAsset
        isReloading = false
        do {
            let loaded = try await store.getAssetDetail(assetId: assetId, userAsset: userAsset)
            let baseCurrencyId = store.getDefaultAssetId()
            let defaultPair = loaded.basePairId
                ?? loaded.pairs.first { $0.assetId == baseCurrencyId }?.assetId
                ?? loaded.pairs.first?.assetId
            detail = loaded
            isLoading = false
            selectedPairId = defaultPair
            if let defaultPair {
                selectPair(defaultPair)
            }
        } catch {
            isLoading = false
            report(error)
        }
    }

    private func handleOverview(_ state: AssetOverviewState) {
        guard assetId != 0, state.assetId == assetId else { return }
        overview = state
        isLoading = false
        if !state.isLoading {
            isReloading = false
            let wasKnown = isHoldingKnown
            let previousSeries = chartSeries
            isHoldingKnown = true
            isHeld = state.error == nil && state.units != 0
            if !wasKnown || chartSeries != previousSeries {
                loadChartIfMissing()
            }
        }
        if let error = state.error {
            lastError = AssetErrorEvent(message: error)
        }
    }

    private func loadPairInfo(_ referenceId: Int32) async {
        let generation = chartGeneration
        do {
            let info: AssetPairDetail
            if let converted = pair(referenceId), converted.converted {
                let rate = try await store.getAssetPairConverted(assetId: assetId, referenceId: referenceId, userAsset: isUserAsset)
                info = AssetPairDetail(
                    mainTicker: detail?.displaySymbol ?? "",
                    mainName: detail?.name ?? "",
                    refTicker: converted.ticker,
                    refName: converted.name,
                    latestRate: rate.latestRate,
                    lastUpdated: rate.lastUpdated,
                    volume: nil,
                    exchange: nil
                )
            } else {
                info = try await store.getAssetPair(assetId: assetId, referenceId: referenceId, userAsset: isUserAsset)
            }
            guard generation == chartGeneration else { return }
            pairInfo = info
        } catch {
            guard generation == chartGeneration else { return }
            report(error)
        }
    }

    private func loadChartIfMissing() {
        let series = chartSeries
        let period = selectedPeriod
        if chartByPeriod[series]?[period] == nil {
            Task { await loadChart(series, period) }
        }
    }

    private func loadChart(_ series: HoldingChartSeries, _ period: ChartPeriod) async {
        guard let referenceId = selectedPairId else { return }
        let generation = chartGeneration
        let assetId = self.assetId
        let range = period.apiRange
        chartErrors[series]?[period] = nil
        do {
            let points: [ChartPoint]
            switch series {
            case .price:
                if pair(referenceId)?.converted == true {
                    points = try await store.getAssetPairConvertedRates(assetId: assetId, referenceId: referenceId, range: range, userAsset: isUserAsset)
                } else {
                    points = try await store.getAssetPairRates(assetId: assetId, referenceId: referenceId, range: range, userAsset: isUserAsset)
                }
            case .value:
                points = try await store.getHoldingValueHistory(assetId: assetId, accountId: nil, currencyId: referenceId, range: range)
            case .units:
                points = try await store.getHoldingUnitsHistory(assetId: assetId, accountId: nil, range: range)
            }
            guard generation == chartGeneration else { return }
            chartByPeriod[series, default: [:]][period] = points
        } catch {
            guard generation == chartGeneration else { return }
            chartErrors[series, default: [:]][period] = error.friendlyMessage
            report(error)
        }
    }

    private func pair(_ referenceId: Int32?) -> AssetPairRef? {
        guard let referenceId else { return nil }
        return detail?.pairs.first { $0.assetId == referenceId }
    }

    private func report(_ error: Error) {
        lastError = AssetErrorEvent(error)
    }
}
