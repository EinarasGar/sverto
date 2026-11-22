import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct EditPropertyViewModelTests {
    @Test func boughtOnStartsAtThePurchaseDate() async {
        let store = EditPropertyStore(noHandle: .init())
        let model = makeModel(store)

        await model.loadPurchaseDate()

        #expect(model.purchaseDate == Date(timeIntervalSince1970: 1_700_000_000))
        #expect(store.purchaseLookups == ["house/7"])
        #expect(model.canSave)
    }

    @Test func savingWithTheSameDateOnlyUpdatesNameAndShare() async {
        let store = EditPropertyStore(noHandle: .init())
        let model = makeModel(store)
        await model.loadPurchaseDate()
        model.name = " iOS Home "
        model.ownershipShare = 0.6

        let saved = await model.save()

        #expect(saved)
        #expect(store.calls == ["update iOS Home 0.6"])
    }

    @Test func aChangedDateMovesThePurchaseAfterTheNameAndShare() async {
        let store = EditPropertyStore(noHandle: .init())
        let model = makeModel(store)
        await model.loadPurchaseDate()
        model.purchaseDate = Date(timeIntervalSince1970: 1_600_000_000)

        let saved = await model.save()

        #expect(saved)
        #expect(store.calls == ["update iOS House 0.5", "purchase house/7/45 -> 1600000000"])
    }

    @Test func aFuturePurchaseDateIsClampedToNow() async throws {
        let store = EditPropertyStore(noHandle: .init())
        let model = makeModel(store)
        await model.loadPurchaseDate()
        model.purchaseDate = Date().addingTimeInterval(86_400)
        let before = Int64(Date().timeIntervalSince1970)

        _ = await model.save()

        let moved = try #require(store.movedDates.first)
        #expect(moved >= before && moved <= Int64(Date().timeIntervalSince1970))
    }

    @Test func anUnknownPurchaseDateStillSavesNameAndShare() async {
        let store = EditPropertyStore(noHandle: .init())
        store.failLookup = true
        let model = makeModel(store)
        await model.loadPurchaseDate()

        #expect(model.purchaseDateError == "no purchase")
        let saved = await model.save()

        #expect(saved)
        #expect(store.calls == ["update iOS House 0.5"])
    }

    @Test func aFailedMoveKeepsTheSheetOpenWithTheReason() async {
        let store = EditPropertyStore(noHandle: .init())
        store.failMove = true
        let model = makeModel(store)
        await model.loadPurchaseDate()
        model.purchaseDate = Date(timeIntervalSince1970: 1_600_000_000)

        let saved = await model.save()

        #expect(!saved)
        #expect(model.saveError == "conflict")
        #expect(!model.isSaving)
    }

    private func makeModel(_ store: EditPropertyStore) -> EditPropertyViewModel {
        let summary = makeSummary()
        return EditPropertyViewModel(
            store: store,
            propertyId: "property-1",
            account: AccountListItem(
                id: "house",
                name: "iOS House",
                accountTypeId: 9,
                liquidityTypeId: 1,
                ownershipShare: 0.5,
                balance: nil,
                unrealizedGain: nil,
                holdingsCount: nil,
                suggestedCurrency: nil,
                property: summary,
                linkedPropertyId: nil
            ),
            summary: summary
        )
    }
}

nonisolated private final class EditPropertyStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var recorded: [String] = []
    private var lookups: [String] = []
    private var moved: [Int64] = []
    private var lookupFails = false
    private var moveFails = false

    var calls: [String] { lock.withLock { recorded } }
    var purchaseLookups: [String] { lock.withLock { lookups } }
    var movedDates: [Int64] { lock.withLock { moved } }

    var failLookup: Bool {
        get { lock.withLock { lookupFails } }
        set { lock.withLock { lookupFails = newValue } }
    }

    var failMove: Bool {
        get { lock.withLock { moveFails } }
        set { lock.withLock { moveFails = newValue } }
    }

    override func getPropertyPurchaseDate(accountId: String, assetId: Int32) async throws -> Int64 {
        lock.withLock { lookups.append("\(accountId)/\(assetId)") }
        if failLookup { throw ApiError.Parse(reason: "no purchase") }
        return 1_700_000_000
    }

    override func updateProperty(propertyId: String, input: UpdatePropertyInput) async throws {
        lock.withLock { recorded.append("update \(input.name) \(input.ownershipShare)") }
    }

    override func setPropertyPurchaseDate(accountId: String, assetId: Int32, currencyAssetId: Int32, date: Int64) async throws {
        if failMove { throw ApiError.Server(reason: "conflict", status: 409) }
        lock.withLock {
            moved.append(date)
            recorded.append("purchase \(accountId)/\(assetId)/\(currencyAssetId) -> \(date)")
        }
    }
}
