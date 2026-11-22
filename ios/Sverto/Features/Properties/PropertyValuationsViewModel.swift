import Foundation
import Observation
import SvertoCore

@Observable
final class PropertyValuationsViewModel {
    private(set) var valuations: [ChartPoint] = []
    private(set) var isLoading = true
    private(set) var loadError: String?

    let currencyTicker: String
    private let store: AppStore
    private let assetId: Int32
    private let currencyAssetId: Int32
    private var hasLoaded = false

    init(store: AppStore, assetId: Int32, currencyAssetId: Int32, currencyTicker: String) {
        self.store = store
        self.assetId = assetId
        self.currencyAssetId = currencyAssetId
        self.currencyTicker = currencyTicker
    }

    func start() async {
        guard !hasLoaded else { return }
        hasLoaded = true
        await load()
    }

    func refresh() async {
        await store.refreshProperty(assetId: assetId)
        await load()
    }

    func load() async {
        do {
            let rates = try await store.getAssetPairRates(
                assetId: assetId,
                referenceId: currencyAssetId,
                range: ChartPeriod.all.apiRange,
                userAsset: true
            )
            valuations = rates.sorted { $0.timestamp > $1.timestamp }
            loadError = nil
        } catch {
            loadError = error.friendlyMessage
        }
        isLoading = false
    }

    func add(value: Double, on date: Date) async throws {
        try await store.addPropertyValuation(
            assetId: assetId,
            currencyAssetId: currencyAssetId,
            date: Self.timestamp(date),
            value: value
        )
        await load()
    }

    func edit(_ valuation: ChartPoint, value: Double, on date: Date) async throws {
        try await store.editPropertyValuation(
            assetId: assetId,
            currencyAssetId: currencyAssetId,
            oldDate: valuation.timestamp,
            newDate: Self.timestamp(date),
            value: value
        )
        await load()
    }

    func delete(_ valuation: ChartPoint) async -> String? {
        do {
            try await store.deletePropertyValuation(assetId: assetId, currencyAssetId: currencyAssetId, date: valuation.timestamp)
            await load()
            return nil
        } catch {
            return error.friendlyMessage
        }
    }

    func deleteMessage(for valuation: ChartPoint) -> String {
        let removal = "This removes the \(Money.format(valuation.value, ticker: currencyTicker)) valuation from \(DateText.mediumDate(epochSeconds: valuation.timestamp))."
        guard valuations.count == 1 else { return removal }
        return "\(removal) The property will have no value until you add a new one."
    }

    private static func timestamp(_ date: Date) -> Int64 {
        Int64(min(date, Date()).timeIntervalSince1970)
    }
}
