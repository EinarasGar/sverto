import SwiftUI

struct TypeNameSheet: View {
    private let isEdit: Bool
    private let initialName: String
    private let onSave: (String) async throws -> Void

    @State private var name: String
    @State private var isSaving = false
    @State private var saveError: String?
    @State private var isConfirmingDiscard = false
    @FocusState private var isNameFocused: Bool
    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    init(initialName: String, isEdit: Bool, onSave: @escaping (String) async throws -> Void) {
        self.isEdit = isEdit
        self.initialName = initialName
        self.onSave = onSave
        _name = State(initialValue: initialName)
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var hasChanges: Bool { trimmedName != initialName.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        NavigationStack {
            Form {
                if let saveError {
                    Section {
                        FormErrorRow(message: saveError)
                    }
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text("Name")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    TextField("Name", text: $name)
                        .focused($isNameFocused)
                        .textInputAutocapitalization(.sentences)
                        .submitLabel(.done)
                        .onSubmit(save)
                }
            }
            .disabled(isSaving)
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle(isEdit ? "Edit type" : "New type")
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
                    .disabled(trimmedName.isEmpty || isSaving)
                    .accessibilityLabel(isSaving ? "Saving type" : "Save type")
                }
            }
            .onChange(of: name) { _, newValue in
                if newValue.count > 50 {
                    name = String(newValue.prefix(50))
                }
            }
            .onAppear { isNameFocused = true }
            .modifier(discardDialog(.screen))
        }
        .interactiveDismissDisabled(hasChanges || isSaving)
        .presentationDetents(dynamicTypeSize.isAccessibilitySize ? [.large] : [.medium, .large])
    }

    private func discardDialog(_ source: DialogSource) -> some ViewModifier {
        ConfirmationDialog(source, title: "Discard changes?", isPresented: $isConfirmingDiscard) {
            Button("Discard changes", role: .destructive) { dismiss() }
            Button("Keep editing", role: .cancel) {}
        }
    }

    private func cancel() {
        guard !isSaving else { return }
        isNameFocused = false
        if hasChanges {
            isConfirmingDiscard = true
        } else {
            dismiss()
        }
    }

    private func save() {
        guard !trimmedName.isEmpty, !isSaving else { return }
        isNameFocused = false
        isSaving = true
        saveError = nil
        let submittedName = trimmedName
        Task {
            defer { isSaving = false }
            do {
                try await onSave(submittedName)
                dismiss()
            } catch {
                Haptics.error()
                saveError = error.friendlyMessage
                AccessibilityNotification.Announcement(error.friendlyMessage).post()
            }
        }
    }
}
