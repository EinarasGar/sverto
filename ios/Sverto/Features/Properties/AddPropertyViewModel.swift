import Foundation
import Observation
import SvertoCore

@Observable
final class AddPropertyViewModel {
    var kind = PropertyKind.realEstate
    var name = ""
    var currency: AssetItem?
    var purchaseDate = Date()
    var purchasePriceText = ""
    var valueText = ""
    var ownershipShare = 1.0
    private(set) var loanAccountIds: [String] = []
    private(set) var loanCandidates: [AccountListItem] = []
    private(set) var loanCandidatesError: String?
    private(set) var isSaving = false
    private(set) var saveError: String?

    private let store: AppStore
    private var hasStarted = false

    init(store: AppStore) {
        self.store = store
    }

    var isBoughtToday: Bool { Calendar.current.isDateInToday(purchaseDate) }

    var input: CreatePropertyInput? {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty, let currency, let price = RateInput.parse(purchasePriceText) else { return nil }
        let value: Double
        let purchase: PropertyPurchaseInput?
        if isBoughtToday {
            value = price
            purchase = nil
        } else {
            guard let valueToday = RateInput.parse(valueText) else { return nil }
            value = valueToday
            purchase = PropertyPurchaseInput(date: Int64(purchaseDate.timeIntervalSince1970), price: price)
        }
        return CreatePropertyInput(
            kind: kind,
            name: trimmedName,
            currencyAssetId: currency.id,
            ownershipShare: ownershipShare,
            value: value,
            purchase: purchase,
            loanAccountIds: loanAccountIds
        )
    }

    var canSave: Bool { input != nil && !isSaving }

    var hasChanges: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !purchasePriceText.isEmpty || !valueText.isEmpty
            || !isBoughtToday || !loanAccountIds.isEmpty || ownershipShare != 1 || kind != .realEstate
    }

    func start() async {
        guard !hasStarted else { return }
        hasStarted = true
        async let candidates: Void = loadLoanCandidates()
        if let baseId = store.getDefaultAssetId() {
            let currencies = (try? await store.getAllCurrencies()) ?? []
            if currency == nil {
                currency = currencies.first { $0.id == baseId }
            }
        }
        await candidates
    }

    private func loadLoanCandidates() async {
        do {
            loanCandidates = try await store.getLoanCandidates()
        } catch {
            loanCandidatesError = error.friendlyMessage
        }
    }

    func toggleLoan(_ accountId: String) {
        if let index = loanAccountIds.firstIndex(of: accountId) {
            loanAccountIds.remove(at: index)
        } else {
            loanAccountIds.append(accountId)
        }
    }

    func save() async -> Bool {
        guard !isSaving, let input else { return false }
        isSaving = true
        saveError = nil
        do {
            _ = try await store.createProperty(input: input)
            isSaving = false
            return true
        } catch {
            saveError = error.friendlyMessage
            isSaving = false
            return false
        }
    }
}
