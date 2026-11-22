import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct AddPropertyViewModelTests {
    @Test func startsBoughtTodayWithTheBaseCurrencyAndFullOwnership() async {
        let model = AddPropertyViewModel(store: AddPropertyStore(noHandle: .init()))

        await model.start()

        #expect(model.currency?.ticker == "GBP")
        #expect(model.loanCandidates.map(\.id) == ["car-loan"])
        #expect(model.ownershipShare == 1)
        #expect(model.kind == .realEstate)
        #expect(model.isBoughtToday)
        #expect(!model.hasChanges)
    }

    @Test func boughtTodayNeedsOnlyAPositivePriceAndSendsItAsTheValue() async {
        let model = AddPropertyViewModel(store: AddPropertyStore(noHandle: .init()))
        await model.start()

        model.name = "  "
        model.purchasePriceText = "900"
        #expect(!model.canSave)

        model.name = "iOS Bike"
        model.purchasePriceText = "0"
        #expect(!model.canSave)

        model.purchasePriceText = "900,50"
        #expect(model.input?.value == 900.5)
        #expect(model.input?.purchase == nil)
    }

    @Test func boughtEarlierNeedsTheValueTodayAndSendsThePurchase() async {
        let model = AddPropertyViewModel(store: AddPropertyStore(noHandle: .init()))
        await model.start()
        model.name = "iOS Boat"
        model.purchaseDate = Date(timeIntervalSince1970: 1_740_787_200)
        model.purchasePriceText = "20000"

        #expect(!model.isBoughtToday)
        #expect(model.hasChanges)
        #expect(!model.canSave)

        model.valueText = "18000"

        #expect(model.input?.value == 18_000)
        #expect(model.input?.purchase == PropertyPurchaseInput(date: 1_740_787_200, price: 20_000))
    }

    @Test func aValueTodayTypedBeforeSwitchingBackToTodayIsIgnored() async {
        let model = AddPropertyViewModel(store: AddPropertyStore(noHandle: .init()))
        await model.start()
        model.name = "iOS Bike"
        model.purchasePriceText = "900"
        model.purchaseDate = Date(timeIntervalSince1970: 1_740_787_200)
        model.valueText = "800"

        model.purchaseDate = Date()

        #expect(model.input?.value == 900)
        #expect(model.input?.purchase == nil)
    }

    @Test func savingSendsTheChosenKindShareAndLoans() async {
        let store = AddPropertyStore(noHandle: .init())
        let model = AddPropertyViewModel(store: store)
        await model.start()
        model.kind = .vehicle
        model.name = " iOS Car "
        model.purchasePriceText = "18000"
        model.ownershipShare = 0.5
        model.toggleLoan("car-loan")
        model.toggleLoan("other-loan")
        model.toggleLoan("other-loan")

        let saved = await model.save()

        #expect(saved)
        #expect(store.created == [CreatePropertyInput(
            kind: .vehicle,
            name: "iOS Car",
            currencyAssetId: 45,
            ownershipShare: 0.5,
            value: 18_000,
            purchase: nil,
            loanAccountIds: ["car-loan"]
        )])
    }

    @Test func aRejectedSaveShowsTheServerMessage() async {
        let store = AddPropertyStore(noHandle: .init())
        store.rejectWith = ApiError.Server(reason: "An account with this name already exists", status: 409)
        let model = AddPropertyViewModel(store: store)
        await model.start()
        model.name = "iOS House"
        model.purchasePriceText = "328000"

        let saved = await model.save()

        #expect(!saved)
        #expect(model.saveError == "An account with this name already exists")
        #expect(!model.isSaving)
    }
}

nonisolated private final class AddPropertyStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var inputs: [CreatePropertyInput] = []
    private var rejection: ApiError?

    var created: [CreatePropertyInput] { lock.withLock { inputs } }

    var rejectWith: ApiError? {
        get { lock.withLock { rejection } }
        set { lock.withLock { rejection = newValue } }
    }

    override func getDefaultAssetId() -> Int32? { 45 }

    override func getAllCurrencies() async throws -> [AssetItem] {
        [AssetItem(id: 1, name: "US Dollar", ticker: "USD"), AssetItem(id: 45, name: "British Pound", ticker: "GBP")]
    }

    override func getLoanCandidates() async throws -> [AccountListItem] {
        [makeAccount("car-loan", type: 8)]
    }

    override func createProperty(input: CreatePropertyInput) async throws -> String {
        if let rejectWith { throw rejectWith }
        lock.withLock { inputs.append(input) }
        return "property-1"
    }
}
