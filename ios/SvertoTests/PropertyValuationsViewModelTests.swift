import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct PropertyValuationsViewModelTests {
    @Test func listsEveryValuationNewestFirst() async {
        let store = ValuationsStore(noHandle: .init())
        let model = makeModel(store)

        await model.start()

        #expect(model.valuations.map(\.timestamp) == [3_000, 2_000, 1_000])
        #expect(store.requestedRanges == ["all"])
    }

    @Test func editingSendsTheOriginalAndTheNewDate() async throws {
        let store = ValuationsStore(noHandle: .init())
        let model = makeModel(store)
        await model.start()

        try await model.edit(ChartPoint(timestamp: 2_000, value: 310_000), value: 315_000, on: Date(timeIntervalSince1970: 2_500))

        #expect(store.calls == ["edit 2000 -> 2500 = 315000.0"])
        #expect(store.requestedRanges.count == 2)
    }

    @Test func aFutureDateIsClampedToNow() async throws {
        let store = ValuationsStore(noHandle: .init())
        let model = makeModel(store)
        await model.start()
        let before = Int64(Date().timeIntervalSince1970)

        try await model.add(value: 320_000, on: Date().addingTimeInterval(86_400))

        let added = try #require(store.addedDates.first)
        #expect(added >= before && added <= Int64(Date().timeIntervalSince1970))
    }

    @Test func deletingSendsTheValuationsOwnDate() async {
        let store = ValuationsStore(noHandle: .init())
        let model = makeModel(store)
        await model.start()

        let error = await model.delete(ChartPoint(timestamp: 1_000, value: 296_000))

        #expect(error == nil)
        #expect(store.calls == ["delete 1000"])
    }

    @Test func deletingTheLastValuationWarnsThePropertyLosesItsValue() async {
        let store = ValuationsStore(noHandle: .init())
        let model = makeModel(store)
        await model.start()
        let valuation = ChartPoint(timestamp: 1_000, value: 296_000)

        #expect(!model.deleteMessage(for: valuation).contains("no value"))

        store.rates = [valuation]
        await model.load()

        #expect(model.deleteMessage(for: valuation).hasSuffix("The property will have no value until you add a new one."))
    }

    private func makeModel(_ store: ValuationsStore) -> PropertyValuationsViewModel {
        PropertyValuationsViewModel(store: store, assetId: 7, currencyAssetId: 45, currencyTicker: "GBP")
    }
}

nonisolated private final class ValuationsStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var stored = [
        ChartPoint(timestamp: 1_000, value: 296_000),
        ChartPoint(timestamp: 3_000, value: 328_000),
        ChartPoint(timestamp: 2_000, value: 310_000),
    ]
    private var recorded: [String] = []
    private var ranges: [String] = []
    private var added: [Int64] = []

    var rates: [ChartPoint] {
        get { lock.withLock { stored } }
        set { lock.withLock { stored = newValue } }
    }

    var calls: [String] { lock.withLock { recorded } }
    var requestedRanges: [String] { lock.withLock { ranges } }
    var addedDates: [Int64] { lock.withLock { added } }

    override func getAssetPairRates(assetId: Int32, referenceId: Int32, range: String, userAsset: Bool) async throws -> [ChartPoint] {
        lock.withLock {
            ranges.append(range)
            return stored
        }
    }

    override func addPropertyValuation(assetId: Int32, currencyAssetId: Int32, date: Int64, value: Double) async throws {
        lock.withLock { added.append(date) }
    }

    override func editPropertyValuation(assetId: Int32, currencyAssetId: Int32, oldDate: Int64, newDate: Int64, value: Double) async throws {
        lock.withLock { recorded.append("edit \(oldDate) -> \(newDate) = \(value)") }
    }

    override func deletePropertyValuation(assetId: Int32, currencyAssetId: Int32, date: Int64) async throws {
        lock.withLock { recorded.append("delete \(date)") }
    }
}
