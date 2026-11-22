import Foundation
import Observation
import SvertoCore

@Observable
final class PropertyDetailViewModel {
    private(set) var detail: PropertyDetail?
    private(set) var isLoading = true
    private(set) var loadError: String?
    private(set) var currencyTicker: String?
    private(set) var currencyError: String?
    private(set) var valueChart: PropertyValueChart?
    private(set) var valueChartError: String?
    private(set) var unlinkingLoanId: String?

    let propertyId: String
    private let store: AppStore
    private var hasLoaded = false

    init(store: AppStore, propertyId: String) {
        self.store = store
        self.propertyId = propertyId
    }

    var summary: PropertySummary? { detail?.summary }

    var isJoint: Bool { (detail?.account.ownershipShare ?? 1) < 1 }

    func appear() async {
        guard hasLoaded else {
            hasLoaded = true
            await load()
            return
        }
        await reloadWithChart()
    }

    func reloadWithChart() async {
        await reload()
        await loadValueChart()
    }

    func retry() async {
        await load()
    }

    func refresh() async {
        guard let summary else { return }
        await store.refreshProperty(assetId: summary.assetId)
        await reload()
        await loadValueChart()
    }

    func reload() async {
        do {
            detail = try await store.getPropertyDetail(propertyId: propertyId)
            loadError = nil
        } catch {
            loadError = error.friendlyMessage
        }
    }

    func retryValueChart() async {
        await loadValueChart()
    }

    func retryCurrency() async {
        await loadCurrency()
    }

    func addValuation(_ value: Double, on date: Date) async throws {
        guard let summary else { return }
        try await store.addPropertyValuation(
            assetId: summary.assetId,
            currencyAssetId: summary.currencyAssetId,
            date: Int64(date.timeIntervalSince1970),
            value: value
        )
        await reload()
        await loadValueChart()
    }

    func linkLoan(accountId: String) async throws {
        try await store.linkPropertyLoan(propertyId: propertyId, accountId: accountId)
        await reload()
    }

    func unlinkLoan(accountId: String) async -> String? {
        unlinkingLoanId = accountId
        defer { unlinkingLoanId = nil }
        do {
            try await store.unlinkPropertyLoan(propertyId: propertyId, accountId: accountId)
            await reload()
            return nil
        } catch {
            return error.friendlyMessage
        }
    }

    private func load() async {
        isLoading = true
        await reload()
        isLoading = false
        guard detail != nil else { return }
        await loadValueChart()
        await loadCurrency()
    }

    private func loadValueChart() async {
        guard let summary else { return }
        do {
            valueChart = try await store.getPropertyValueChart(
                assetId: summary.assetId,
                currencyAssetId: summary.currencyAssetId,
                currentValue: summary.value
            )
            valueChartError = nil
        } catch {
            valueChartError = error.friendlyMessage
        }
    }

    private func loadCurrency() async {
        guard let summary else { return }
        currencyError = nil
        do {
            let currencies = try await store.getAllCurrencies()
            currencyTicker = currencies.first { $0.id == summary.currencyAssetId }?.ticker
            if currencyTicker == nil {
                currencyError = "This property's currency isn't available."
            }
        } catch {
            currencyError = error.friendlyMessage
        }
    }
}
