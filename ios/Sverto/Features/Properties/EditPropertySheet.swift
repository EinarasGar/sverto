import SvertoCore
import SwiftUI

struct EditPropertySheet: View {
    let propertyId: String
    let account: AccountListItem
    let summary: PropertySummary
    let onSaved: () async -> Void

    @Environment(AppServices.self) private var services

    var body: some View {
        EditPropertyForm(
            model: EditPropertyViewModel(store: services.store, propertyId: propertyId, account: account, summary: summary),
            onSaved: onSaved
        )
    }
}

private struct EditPropertyForm: View {
    @State private var model: EditPropertyViewModel
    let onSaved: () async -> Void

    @Environment(\.dismiss) private var dismiss
    @FocusState private var isNameFocused: Bool

    init(model: EditPropertyViewModel, onSaved: @escaping () async -> Void) {
        _model = State(initialValue: model)
        self.onSaved = onSaved
    }

    var body: some View {
        NavigationStack {
            Form {
                if let error = model.saveError {
                    Section("Couldn't save property") {
                        FormErrorRow(message: error)
                    }
                }
                Section("Name") {
                    TextField("Name", text: $model.name, prompt: Text("Property name"))
                        .focused($isNameFocused)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.done)
                }
                Section {
                    OwnershipShareSlider(share: $model.ownershipShare)
                } header: {
                    Text("Your ownership share")
                }
                Section {
                    if model.isLoadingPurchaseDate {
                        ProgressView()
                            .frame(maxWidth: .infinity)
                    } else if let error = model.purchaseDateError {
                        FormErrorRow(message: "Couldn't load the purchase date. \(error)")
                        Button("Retry") {
                            Task { await model.loadPurchaseDate() }
                        }
                    } else {
                        DatePicker("Bought on", selection: $model.purchaseDate, in: ...Date(), displayedComponents: .date)
                    }
                } header: {
                    Text("Purchase")
                } footer: {
                    Text("Changing the date moves the purchase and the valuation recorded on it.")
                }
            }
            .disabled(model.isSaving)
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle("Edit property")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .tint(.primary)
                        .disabled(model.isSaving)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(action: save) {
                        if model.isSaving {
                            ProgressView()
                        } else {
                            Text("Save")
                        }
                    }
                    .disabled(!model.canSave)
                    .accessibilityLabel(model.isSaving ? "Saving property" : "Save property")
                }
            }
            .onChange(of: model.saveError) { _, message in
                if let message { AccessibilityNotification.Announcement(message).post() }
            }
            .task { await model.loadPurchaseDate() }
        }
        .presentationDetents([.large])
        .interactiveDismissDisabled(model.isSaving)
    }

    private func save() {
        isNameFocused = false
        Task {
            if await model.save() {
                await onSaved()
                dismiss()
            }
        }
    }
}
