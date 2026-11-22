import Foundation
import Observation
import SvertoCore

enum AccountSubmitState: Equatable {
    case idle
    case saving
    case error(String)
}

@Observable
final class AddAccountViewModel {
    nonisolated static let liquidLiquidityId: Int32 = 1

    var name = ""
    var selectedTypeId: Int32?
    var selectedLiquidityId: Int32? = liquidLiquidityId
    var ownershipShare = 1.0
    private(set) var identifiers: [AccountIdentifier] = []
    private(set) var belongsToProperty = false
    private(set) var accountTypes: [AccountTypeItem] = []
    private(set) var isLoading = true
    private(set) var isLoadingTypes = true
    private(set) var submitState: AccountSubmitState = .idle
    private(set) var editLoadError: String?
    private(set) var accountTypesError: String?

    private let store: AppStore
    private let editingAccountId: String?
    private var hasStarted = false
    private var initialSnapshot: AccountFormSnapshot?

    init(store: AppStore, editingAccountId: String?) {
        self.store = store
        self.editingAccountId = editingAccountId
    }

    var isEditing: Bool { editingAccountId != nil }
    var isSaving: Bool { submitState == .saving }

    var isValid: Bool {
        !trimmedName.isEmpty && selectedTypeId != nil && selectedLiquidityId != nil
    }

    var canSave: Bool {
        isValid && !isSaving && !isLoading && editLoadError == nil
            && !isLoadingTypes && accountTypesError == nil && !accountTypes.isEmpty
    }

    var hasChanges: Bool {
        guard let initialSnapshot else { return false }
        return snapshot != initialSnapshot
    }

    var showsNameError: Bool {
        !name.isEmpty && trimmedName.isEmpty
    }

    private var trimmedName: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var snapshot: AccountFormSnapshot {
        AccountFormSnapshot(
            name: trimmedName,
            typeId: selectedTypeId,
            liquidityId: selectedLiquidityId,
            ownershipShare: ownershipShare,
            identifiers: Set(identifiers)
        )
    }

    func start() async {
        guard !hasStarted else { return }
        hasStarted = true
        await loadInitialData()
    }

    func retryLoading() async {
        guard !isLoading, editLoadError != nil else { return }
        await loadInitialData()
    }

    func retryAccountTypes() async {
        guard !isLoadingTypes, !isSaving else { return }
        await loadAccountTypes()
    }

    func selectType(_ typeId: Int32) {
        selectedTypeId = typeId
    }

    func selectLiquid() {
        selectedLiquidityId = Self.liquidLiquidityId
    }

    func addIdentifier(kind: AccountIdentifierKind, raw: String) -> String? {
        let value = kind.normalize(raw)
        if let error = kind.validationError(for: value) {
            return error
        }
        if identifiers.contains(where: { $0.kind == kind.rawValue && $0.value == value }) {
            return "Already added."
        }
        identifiers.append(AccountIdentifier(kind: kind.rawValue, value: value))
        return nil
    }

    func removeIdentifier(_ identifier: AccountIdentifier) {
        identifiers.removeAll { $0 == identifier }
    }

    func save() async -> Bool {
        guard canSave, let typeId = selectedTypeId, let liquidityId = selectedLiquidityId else { return false }
        submitState = .saving
        do {
            if let editingAccountId {
                try await store.updateAccount(
                    accountId: editingAccountId,
                    input: UpdateAccountInput(
                        name: trimmedName,
                        accountTypeId: typeId,
                        liquidityTypeId: liquidityId,
                        ownershipShare: ownershipShare,
                        identifiers: identifiers
                    )
                )
            } else {
                try await store.createAccount(
                    input: CreateAccountInput(
                        name: trimmedName,
                        accountTypeId: typeId,
                        liquidityTypeId: liquidityId,
                        ownershipShare: ownershipShare,
                        identifiers: identifiers
                    )
                )
            }
            submitState = .idle
            return true
        } catch {
            submitState = .error(error.friendlyMessage)
            return false
        }
    }

    private func loadInitialData() async {
        isLoading = true
        editLoadError = nil
        async let types: Void = loadAccountTypes()
        if let editingAccountId {
            await loadForEdit(editingAccountId)
        }
        await types
        if editLoadError == nil {
            initialSnapshot = snapshot
        }
        isLoading = false
    }

    private func loadAccountTypes() async {
        isLoadingTypes = true
        accountTypesError = nil
        do {
            accountTypes = try await store.getAccountTypes()
        } catch {
            accountTypesError = error.friendlyMessage
        }
        isLoadingTypes = false
    }

    private func loadForEdit(_ accountId: String) async {
        do {
            let account = try await store.getAccount(accountId: accountId)
            name = account.name
            selectedTypeId = account.accountTypeId
            selectedLiquidityId = account.liquidityTypeId
            ownershipShare = account.ownershipShare
            identifiers = account.identifiers
            belongsToProperty = account.belongsToProperty
        } catch {
            editLoadError = error.friendlyMessage
        }
    }
}
