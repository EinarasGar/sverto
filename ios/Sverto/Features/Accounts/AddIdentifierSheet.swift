import SwiftUI

struct AddIdentifierSheet: View {
    let onAdd: (AccountIdentifierKind, String) -> String?

    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var kind: AccountIdentifierKind = .cardLast4
    @State private var value = ""
    @State private var error: String?
    @State private var isConfirmingDiscard = false
    @FocusState private var isValueFocused: Bool

    private var isValueBlank: Bool {
        value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Type", selection: $kind) {
                        ForEach(AccountIdentifierKind.allCases) { kind in
                            Text(kind.label).tag(kind)
                        }
                    }
                    VStack(alignment: .leading, spacing: 6) {
                        Text(kind.valueFieldLabel)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                        TextField(kind.valueFieldLabel, text: $value)
                            .keyboardType(kind.usesNumericKeyboard ? .numberPad : .asciiCapable)
                            .textInputAutocapitalization(kind == .iban ? .characters : .never)
                            .autocorrectionDisabled()
                            .focused($isValueFocused)
                            .submitLabel(.done)
                            .accessibilityLabel(kind.valueFieldLabel)
                            .onSubmit(add)
                    }
                } footer: {
                    if let error {
                        Text(error)
                            .foregroundStyle(.red)
                    } else if kind == .cardLast4 {
                        Text("Enter the last 4 digits printed on your card.")
                    }
                }
            }
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle("Add identifier")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: cancel)
                        .tint(.primary)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add", action: add)
                        .disabled(isValueBlank)
                }
            }
            .onChange(of: kind) {
                value = ""
                error = nil
            }
            .onChange(of: value) {
                error = nil
            }
            .onAppear { isValueFocused = true }
            .confirmationDialog("Discard changes?", isPresented: $isConfirmingDiscard, titleVisibility: .visible) {
                Button("Discard changes", role: .destructive) { dismiss() }
                Button("Keep editing", role: .cancel) {}
            }
        }
        .interactiveDismissDisabled(!isValueBlank)
        .presentationDetents(dynamicTypeSize.isAccessibilitySize ? [.large] : [.medium, .large])
    }

    private func cancel() {
        isValueFocused = false
        if isValueBlank {
            dismiss()
        } else {
            isConfirmingDiscard = true
        }
    }

    private func add() {
        guard !isValueBlank else { return }
        if let validationError = onAdd(kind, value) {
            error = validationError
            Haptics.error()
            AccessibilityNotification.Announcement(validationError).post()
        } else {
            dismiss()
        }
    }
}
