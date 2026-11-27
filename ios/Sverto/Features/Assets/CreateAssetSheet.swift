import SvertoCore
import SwiftUI

struct CreateAssetSheet: View {
    let assetTypes: [AssetTypeOption]
    let onSubmit: (_ name: String, _ ticker: String, _ assetType: Int32, _ baseAssetId: Int32) async throws -> Void
    private let initialTypeId: Int32?

    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var name = ""
    @State private var ticker = ""
    @State private var typeId: Int32?
    @State private var baseAsset: AssetItem?
    @State private var isBasePickerPresented = false
    @State private var isSaving = false
    @State private var submitError: String?
    @State private var isDiscardPresented = false
    @FocusState private var isNameFocused: Bool
    @FocusState private var isTickerFocused: Bool

    private static let nameLimit = 200
    private static let tickerLimit = 20

    init(assetTypes: [AssetTypeOption], onSubmit: @escaping (_ name: String, _ ticker: String, _ assetType: Int32, _ baseAssetId: Int32) async throws -> Void) {
        self.assetTypes = assetTypes
        self.onSubmit = onSubmit
        initialTypeId = assetTypes.first?.id
        _typeId = State(initialValue: assetTypes.first?.id)
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var trimmedTicker: String { ticker.trimmingCharacters(in: .whitespacesAndNewlines) }

    private var canSave: Bool {
        !trimmedName.isEmpty && !trimmedTicker.isEmpty && typeId != nil && baseAsset != nil
    }

    private var hasChanges: Bool {
        !trimmedName.isEmpty || !trimmedTicker.isEmpty || typeId != initialTypeId || baseAsset != nil
    }

    private var baseAssetLabel: String {
        guard let baseAsset else { return "Select base asset" }
        return "\(baseAsset.ticker) — \(baseAsset.name)"
    }

    var body: some View {
        NavigationStack {
            ScrollViewReader { proxy in
                Form {
                    if let submitError {
                        Section("Couldn't save asset") {
                            FormErrorRow(message: submitError)
                        }
                        .id("asset-save-error")
                    }
                    Section {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Name")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            TextField("Name", text: $name)
                                .focused($isNameFocused)
                                .submitLabel(.next)
                                .onSubmit { isTickerFocused = true }
                                .onChange(of: name) { _, value in name = String(value.prefix(Self.nameLimit)) }
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Ticker")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            TextField("Ticker", text: $ticker)
                                .autocorrectionDisabled()
                                .textInputAutocapitalization(.characters)
                                .focused($isTickerFocused)
                                .submitLabel(.done)
                                .onSubmit { isTickerFocused = false }
                                .onChange(of: ticker) { _, value in ticker = String(value.prefix(Self.tickerLimit)) }
                        }
                    }
                    Section {
                        AssetTypeDropdown(assetTypes: assetTypes, selectedId: $typeId)
                        LabeledPickerField(label: "Base pair asset", value: baseAssetLabel, isPlaceholder: baseAsset == nil) {
                            isBasePickerPresented = true
                        }
                    }
                }
                .disabled(isSaving)
                .scrollDismissesKeyboard(.interactively)
                .onChange(of: submitError) { _, message in
                    guard let message else { return }
                    proxy.scrollTo("asset-save-error", anchor: .top)
                    AccessibilityNotification.Announcement(message).post()
                }
            }
            .navigationTitle("New asset")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: cancel)
                        .modifier(discardDialog(.trigger))
                        .tint(.primary)
                        .disabled(isSaving)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(action: save) {
                        if isSaving {
                            ProgressView()
                        } else {
                            Text("Save")
                        }
                    }
                    .disabled(!canSave || isSaving)
                    .accessibilityLabel(isSaving ? "Saving asset" : "Save asset")
                }
            }
            .sheet(isPresented: $isBasePickerPresented) {
                AssetPickerSheet(title: "Select base asset") { asset in
                    baseAsset = asset
                }
            }
            .modifier(discardDialog(.screen))
        }
        .presentationDetents(dynamicTypeSize.isAccessibilitySize ? [.large] : [.medium, .large])
        .interactiveDismissDisabled(hasChanges || isSaving)
    }

    private func save() {
        guard !isSaving, canSave, let typeId, let baseAsset else { return }
        let name = trimmedName
        let ticker = trimmedTicker
        isNameFocused = false
        isTickerFocused = false
        isSaving = true
        submitError = nil
        Task {
            do {
                try await onSubmit(name, ticker, typeId, baseAsset.id)
                Haptics.success()
                dismiss()
            } catch {
                Haptics.error()
                submitError = error.friendlyMessage
                isSaving = false
            }
        }
    }

    private func discardDialog(_ source: DialogSource) -> some ViewModifier {
        ConfirmationDialog(source, title: "Discard changes?", isPresented: $isDiscardPresented) {
            Button("Discard changes", role: .destructive) { dismiss() }
            Button("Keep editing", role: .cancel) {}
        } message: {
            Text("This asset hasn't been saved.")
        }
    }

    private func cancel() {
        guard !isSaving else { return }
        isNameFocused = false
        isTickerFocused = false
        if hasChanges {
            isDiscardPresented = true
        } else {
            dismiss()
        }
    }
}
