import Foundation
import Observation
import SvertoCore

@Observable
final class EditPropertyViewModel {
    var name: String
    var ownershipShare: Double
    var purchaseDate = Date()
    private(set) var isLoadingPurchaseDate = true
    private(set) var purchaseDateError: String?
    private(set) var isSaving = false
    private(set) var saveError: String?

    private let store: AppStore
    private let propertyId: String
    private let accountId: String
    private let assetId: Int32
    private let currencyAssetId: Int32
    private var loadedPurchaseDate: Int64?

    init(store: AppStore, propertyId: String, account: AccountListItem, summary: PropertySummary) {
        self.store = store
        self.propertyId = propertyId
        accountId = account.id
        assetId = summary.assetId
        currencyAssetId = summary.currencyAssetId
        name = account.name
        ownershipShare = account.ownershipShare
    }

    var canSave: Bool {
        !trimmedName.isEmpty && !isSaving && !isLoadingPurchaseDate
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }

    func loadPurchaseDate() async {
        isLoadingPurchaseDate = true
        purchaseDateError = nil
        do {
            let date = try await store.getPropertyPurchaseDate(accountId: accountId, assetId: assetId)
            loadedPurchaseDate = date
            purchaseDate = DateText.date(fromUnix: date)
        } catch {
            purchaseDateError = error.friendlyMessage
        }
        isLoadingPurchaseDate = false
    }

    func save() async -> Bool {
        guard canSave else { return false }
        isSaving = true
        saveError = nil
        defer { isSaving = false }
        do {
            try await store.updateProperty(
                propertyId: propertyId,
                input: UpdatePropertyInput(name: trimmedName, ownershipShare: ownershipShare)
            )
            let newPurchaseDate = Int64(min(purchaseDate, Date()).timeIntervalSince1970)
            if let loadedPurchaseDate, newPurchaseDate != loadedPurchaseDate {
                try await store.setPropertyPurchaseDate(
                    accountId: accountId,
                    assetId: assetId,
                    currencyAssetId: currencyAssetId,
                    date: newPurchaseDate
                )
                self.loadedPurchaseDate = newPurchaseDate
            }
            return true
        } catch {
            saveError = error.friendlyMessage
            return false
        }
    }
}
