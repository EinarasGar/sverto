import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct OnboardingViewModelTests {
    private let pound = AssetItem(id: 1, name: "British Pound", ticker: "GBP")
    private let dollar = AssetItem(id: 2, name: "US Dollar", ticker: "USD")

    @Test func failedLoadIsDistinctFromAnEmptyCurrencyResponse() async {
        let store = OnboardingCurrencyStore(noHandle: .init())
        store.setResponse(.failure(.Network(reason: "Offline")))
        let model = OnboardingViewModel(store: store)

        await model.loadCurrencies()

        #expect(model.currencyLoadError == "Couldn't load currencies. Try again.")
        #expect(model.currencyResults.isEmpty)
        #expect(model.selectedCurrency == nil)
        #expect(!model.isLoadingCurrencies)
        #expect(model.error == nil)
    }

    @Test func successfulEmptyLoadHasNoFailure() async {
        let model = OnboardingViewModel(store: OnboardingCurrencyStore(noHandle: .init()))

        await model.loadCurrencies()

        #expect(model.currencyLoadError == nil)
        #expect(model.currencyResults.isEmpty)
        #expect(!model.isLoadingCurrencies)
    }

    @Test func successfulRetryClearsFailureAndUsesTheCurrentSearch() async {
        let store = OnboardingCurrencyStore(noHandle: .init())
        store.setResponse(.failure(.Parse(reason: "Invalid currencies")))
        let model = OnboardingViewModel(store: store)
        await model.loadCurrencies()
        model.onQueryChange("pound")
        store.setResponse(.success([dollar, pound]))

        await model.loadCurrencies()

        #expect(model.currencyLoadError == nil)
        #expect(model.query == "pound")
        #expect(model.currencyResults == [pound])
        #expect(model.selectedCurrency == pound)
        #expect(!model.isLoadingCurrencies)
    }

    @Test func failedReloadPreservesCurrenciesSearchAndSelection() async {
        let store = OnboardingCurrencyStore(noHandle: .init())
        store.setResponse(.success([dollar, pound]))
        let model = OnboardingViewModel(store: store)
        await model.loadCurrencies()
        model.selectCurrency(dollar)
        model.onQueryChange("pound")
        store.setResponse(.failure(.Network(reason: "Offline")))

        await model.loadCurrencies()

        #expect(model.currencyLoadError != nil)
        #expect(model.query == "pound")
        #expect(model.currencyResults == [pound])
        #expect(model.selectedCurrency == dollar)
        model.onQueryChange("USD")
        #expect(model.currencyResults == [dollar])
    }

    @Test func retryKeepsAnExplicitSelectionInsteadOfRestoringTheDefault() async {
        let store = OnboardingCurrencyStore(noHandle: .init())
        store.setResponse(.success([dollar, pound]))
        let model = OnboardingViewModel(store: store)
        await model.loadCurrencies()
        model.selectCurrency(dollar)
        store.setResponse(.failure(.Network(reason: "Offline")))
        await model.loadCurrencies()
        store.setResponse(.success([pound, dollar]))

        await model.loadCurrencies()

        #expect(model.currencyLoadError == nil)
        #expect(model.selectedCurrency == dollar)
        #expect(Set(model.currencyResults.map(\.id)) == [pound.id, dollar.id])
    }

    @Test func aiFeaturesChoiceIsSavedWithTheChosenValue() async {
        let store = OnboardingCurrencyStore(noHandle: .init())
        let model = OnboardingViewModel(store: store)

        let keptOff = await model.setAiFeaturesEnabled(false)
        let turnedOn = await model.setAiFeaturesEnabled(true)

        #expect(keptOff)
        #expect(turnedOn)
        #expect(store.aiFeaturesRequests == [false, true])
        #expect(model.error == nil)
        #expect(!model.isSaving)
    }

    @Test func failedAiFeaturesChoiceSurfacesTheErrorAndDoesNotAdvance() async {
        let store = OnboardingCurrencyStore(noHandle: .init())
        store.setAiFeaturesFailure(.Network(reason: "Offline"))
        let model = OnboardingViewModel(store: store)

        let saved = await model.setAiFeaturesEnabled(true)

        #expect(!saved)
        #expect(model.error == "Offline")
        #expect(!model.isSaving)
        model.clearError()
        #expect(model.error == nil)
    }
}

nonisolated private final class OnboardingCurrencyStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var response: Result<[AssetItem], ApiError> = .success([])
    private var aiFeaturesFailure: ApiError?
    private var aiFeaturesValues: [Bool] = []

    var aiFeaturesRequests: [Bool] { lock.withLock { aiFeaturesValues } }

    func setResponse(_ response: Result<[AssetItem], ApiError>) {
        lock.withLock { self.response = response }
    }

    func setAiFeaturesFailure(_ failure: ApiError) {
        lock.withLock { aiFeaturesFailure = failure }
    }

    override func getAllCurrencies() async throws -> [AssetItem] {
        try lock.withLock { response }.get()
    }

    override func setAiFeaturesEnabled(enabled: Bool) async throws {
        try lock.withLock {
            if let aiFeaturesFailure { throw aiFeaturesFailure }
            aiFeaturesValues.append(enabled)
        }
    }

    override func getOnboardingVersion() -> Int32 { 0 }
    override func getDefaultAssetId() -> Int32? { 1 }
}
