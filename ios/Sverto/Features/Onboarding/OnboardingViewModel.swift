import Foundation
import Observation
import SvertoCore

@Observable
final class OnboardingViewModel {
    let steps: [OnboardingStepId]
    private(set) var query = ""
    private(set) var currencyResults: [AssetItem] = []
    private(set) var selectedCurrency: AssetItem?
    private(set) var isLoadingCurrencies = false
    private(set) var currencyLoadError: String?
    private(set) var isSaving = false
    private(set) var finished = false
    private(set) var error: String?

    private let store: AppStore
    private var allCurrencies: [AssetItem] = []

    init(store: AppStore) {
        self.store = store
        steps = OnboardingSteps.steps(forCompletedVersion: store.getOnboardingVersion())
    }

    func loadCurrencies() async {
        guard !isLoadingCurrencies else { return }
        isLoadingCurrencies = true
        currencyLoadError = nil
        defer { isLoadingCurrencies = false }
        do {
            let loaded = try await store.getAllCurrencies()
            allCurrencies = CurrencyOrdering.order(loaded)
            currencyResults = CurrencySearch.filter(allCurrencies, query: query)
            if selectedCurrency == nil, let defaultId = store.getDefaultAssetId() {
                selectedCurrency = allCurrencies.first { $0.id == defaultId }
            }
        } catch {
            currencyLoadError = "Couldn't load currencies. Try again."
        }
    }

    func onQueryChange(_ newQuery: String) {
        query = newQuery
        currencyResults = CurrencySearch.filter(allCurrencies, query: newQuery)
    }

    func selectCurrency(_ item: AssetItem) {
        selectedCurrency = item
    }

    func setAiFeaturesEnabled(_ enabled: Bool) async -> Bool {
        isSaving = true
        error = nil
        defer { isSaving = false }
        do {
            try await store.setAiFeaturesEnabled(enabled: enabled)
            return true
        } catch {
            self.error = error.friendlyMessage
            return false
        }
    }

    func clearError() {
        error = nil
    }

    func finish() async {
        isSaving = true
        error = nil
        do {
            if let selectedCurrency {
                try await store.updateBaseAsset(assetId: selectedCurrency.id, ticker: selectedCurrency.ticker)
            }
            try await store.setOnboardingVersion(version: OnboardingSteps.currentVersion)
            finished = true
        } catch {
            self.error = error.friendlyMessage
        }
        isSaving = false
    }
}
