import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct PropertyDetailViewModelTests {
    @Test func chartIsTheTrendForThePropertyAssetAtItsCurrentValue() async {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 0.5, value: 328_000, loanBalance: -200_000)
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")

        await model.appear()

        #expect(model.isJoint)
        #expect(model.valueChart == store.valueChart)
        #expect(store.chartRequests == [ChartRequest(assetId: 7, currencyAssetId: 45, currentValue: 328_000)])
    }

    @Test func valuationsUseThePropertysOwnCurrencyNotTheBase() async {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 1, value: 300_000, loanBalance: 0)
        store.currencies = [AssetItem(id: 45, name: "Euro", ticker: "EUR")]
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")

        await model.appear()

        #expect(model.currencyTicker == "EUR")
        #expect(model.currencyError == nil)
    }

    @Test func aMissingCurrencyBlocksOnlyUpdateValueNotTheChart() async {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 1, value: 300_000, loanBalance: 0)
        store.currencies = nil
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")

        await model.appear()

        #expect(model.currencyTicker == nil)
        #expect(model.currencyError == "offline")
        #expect(model.valueChart == store.valueChart)

        store.currencies = [AssetItem(id: 45, name: "Euro", ticker: "EUR")]
        await model.retryCurrency()

        #expect(model.currencyTicker == "EUR")
        #expect(model.currencyError == nil)
    }

    @Test func pullToRefreshDropsThePropertysCachedDataBeforeReloading() async {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 1, value: 300_000, loanBalance: 0)
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")
        await model.appear()
        store.detail = detail(share: 1, value: 320_000, loanBalance: 0)

        await model.refresh()

        #expect(store.recordedCalls.suffix(2) == ["refresh property 7", "detail"])
        #expect(model.summary?.value == 320_000)
        #expect(store.chartRequests.map(\.currentValue) == [300_000, 320_000])
    }

    @Test func addingAValuationPostsARateForThePropertyAssetAndReloads() async throws {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 1, value: 300_000, loanBalance: 0)
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")
        await model.appear()
        store.detail = detail(share: 1, value: 350_000, loanBalance: 0)

        try await model.addValuation(350_000, on: Date(timeIntervalSince1970: 1_700_000_000))

        #expect(store.postedRates == [PostedRate(assetId: 7, currencyAssetId: 45, date: 1_700_000_000, value: 350_000)])
        #expect(model.summary?.value == 350_000)
        #expect(store.chartRequests.map(\.currentValue) == [300_000, 350_000])
    }

    @Test func linkingAndUnlinkingReloadTheDetail() async throws {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 1, value: 300_000, loanBalance: 0)
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")
        await model.appear()

        store.detail = detail(share: 1, value: 300_000, loanBalance: -100_000, loans: ["mortgage"])
        try await model.linkLoan(accountId: "mortgage")
        #expect(model.detail?.loans.map(\.id) == ["mortgage"])

        store.detail = detail(share: 1, value: 300_000, loanBalance: 0)
        let error = await model.unlinkLoan(accountId: "mortgage")

        #expect(error == nil)
        #expect(model.detail?.loans.isEmpty == true)
        #expect(store.recordedCalls.filter { $0 != "detail" } == ["link mortgage", "unlink mortgage"])
    }

    @Test func aFailedUnlinkKeepsTheLoanAndReportsWhy() async {
        let store = PropertyDetailStore(noHandle: .init())
        store.detail = detail(share: 1, value: 300_000, loanBalance: -100_000, loans: ["mortgage"])
        let model = PropertyDetailViewModel(store: store, propertyId: "property-1")
        await model.appear()
        store.failUnlink = true

        let error = await model.unlinkLoan(accountId: "mortgage")

        #expect(error == "not linked")
        #expect(model.detail?.loans.map(\.id) == ["mortgage"])
        #expect(model.unlinkingLoanId == nil)
    }

    private func detail(share: Double, value: Double, loanBalance: Double, loans: [String] = []) -> PropertyDetail {
        let summary = makeSummary(
            loanIds: loans,
            value: value,
            loanBalance: loanBalance,
            equity: value + loanBalance,
            equityShare: value * share + loanBalance
        )
        return PropertyDetail(
            account: makeAccount("house", type: 9, share: share, balance: value, property: summary),
            summary: summary,
            loans: loans.map { makeAccount($0, type: 7, balance: loanBalance, linkedPropertyId: "property-1") },
            baseTicker: "GBP"
        )
    }
}

struct ChartRequest: Equatable {
    let assetId: Int32
    let currencyAssetId: Int32
    let currentValue: Double?
}

struct PostedRate: Equatable {
    let assetId: Int32
    let currencyAssetId: Int32
    let date: Int64
    let value: Double
}

nonisolated private final class PropertyDetailStore: AppStore, @unchecked Sendable {
    let valueChart = PropertyValueChart(
        line: [ChartPoint(timestamp: 1_600_000_000, value: 296_000), ChartPoint(timestamp: 1_700_000_000, value: 328_000)],
        valuations: [ChartPoint(timestamp: 1_600_000_000, value: 296_000)]
    )

    private let lock = NSLock()
    private var current: PropertyDetail?
    private var posted: [PostedRate] = []
    private var calls: [String] = []
    private var requests: [ChartRequest] = []
    private var shouldFailUnlink = false
    private var availableCurrencies: [AssetItem]? = [AssetItem(id: 45, name: "British Pound", ticker: "GBP")]

    var detail: PropertyDetail? {
        get { lock.withLock { current } }
        set { lock.withLock { current = newValue } }
    }

    var failUnlink: Bool {
        get { lock.withLock { shouldFailUnlink } }
        set { lock.withLock { shouldFailUnlink = newValue } }
    }

    var currencies: [AssetItem]? {
        get { lock.withLock { availableCurrencies } }
        set { lock.withLock { availableCurrencies = newValue } }
    }

    var postedRates: [PostedRate] { lock.withLock { posted } }
    var recordedCalls: [String] { lock.withLock { calls } }
    var chartRequests: [ChartRequest] { lock.withLock { requests } }

    override func getPropertyDetail(propertyId: String) async throws -> PropertyDetail {
        lock.withLock { calls.append("detail") }
        guard let detail else { throw ApiError.Parse(reason: "property not found") }
        return detail
    }

    override func refreshProperty(assetId: Int32) async {
        lock.withLock { calls.append("refresh property \(assetId)") }
    }

    override func getAllCurrencies() async throws -> [AssetItem] {
        guard let currencies else { throw ApiError.Network(reason: "offline") }
        return currencies
    }

    override func getPropertyValueChart(assetId: Int32, currencyAssetId: Int32, currentValue: Double?) async throws -> PropertyValueChart {
        lock.withLock { requests.append(ChartRequest(assetId: assetId, currencyAssetId: currencyAssetId, currentValue: currentValue)) }
        return valueChart
    }

    override func addPropertyValuation(assetId: Int32, currencyAssetId: Int32, date: Int64, value: Double) async throws {
        lock.withLock { posted.append(PostedRate(assetId: assetId, currencyAssetId: currencyAssetId, date: date, value: value)) }
    }

    override func linkPropertyLoan(propertyId: String, accountId: String) async throws {
        lock.withLock { calls.append("link \(accountId)") }
    }

    override func unlinkPropertyLoan(propertyId: String, accountId: String) async throws {
        if failUnlink { throw ApiError.Server(reason: "not linked", status: 404) }
        lock.withLock { calls.append("unlink \(accountId)") }
    }
}
