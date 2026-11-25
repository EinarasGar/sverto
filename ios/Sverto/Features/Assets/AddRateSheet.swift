import SwiftUI

enum RateInput {
    nonisolated static func parse(_ text: String) -> Double? {
        let normalized = text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")
        guard let value = Double(normalized), value > 0 else { return nil }
        return value
    }
}

struct AddRateSheet: View {
    let onSubmit: (_ date: Int64, _ rate: Double) async throws -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var rateText = ""
    @State private var isSaving = false
    @State private var submitError: String?
    @State private var isDiscardPresented = false
    @FocusState private var isRateFocused: Bool

    private var rate: Double? { RateInput.parse(rateText) }
    private var showsError: Bool { !rateText.trimmingCharacters(in: .whitespaces).isEmpty && rate == nil }
    private var hasChanges: Bool { !rateText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    var body: some View {
        NavigationStack {
            Form {
                if let submitError {
                    Section("Couldn't save rate") {
                        FormErrorRow(message: submitError)
                    }
                }
                Section {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Rate")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                        TextField("Rate", text: $rateText)
                            .keyboardType(.decimalPad)
                            .focused($isRateFocused)
                            .submitLabel(.done)
                            .onSubmit(save)
                    }
                } footer: {
                    if showsError {
                        Text("Enter a positive number")
                            .foregroundStyle(.red)
                    }
                }
            }
            .disabled(isSaving)
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle("Add rate")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: cancel)
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
                    .disabled(rate == nil || isSaving)
                    .accessibilityLabel(isSaving ? "Saving rate" : "Save rate")
                }
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button("Done") { isRateFocused = false }
                }
            }
            .onAppear { isRateFocused = true }
            .onChange(of: submitError) { _, message in
                if let message { AccessibilityNotification.Announcement(message).post() }
            }
            .confirmationDialog("Discard changes?", isPresented: $isDiscardPresented, titleVisibility: .visible) {
                Button("Discard changes", role: .destructive) { dismiss() }
                Button("Keep editing", role: .cancel) {}
            } message: {
                Text("This rate hasn't been saved.")
            }
        }
        .presentationDetents(dynamicTypeSize.isAccessibilitySize ? [.large] : [.medium, .large])
        .interactiveDismissDisabled(hasChanges || isSaving)
    }

    private func save() {
        guard !isSaving, let rate else { return }
        let timestamp = Int64(Date().timeIntervalSince1970)
        isRateFocused = false
        isSaving = true
        submitError = nil
        Task {
            do {
                try await onSubmit(timestamp, rate)
                Haptics.success()
                dismiss()
            } catch {
                Haptics.error()
                submitError = error.friendlyMessage
                isSaving = false
            }
        }
    }

    private func cancel() {
        guard !isSaving else { return }
        isRateFocused = false
        if hasChanges {
            isDiscardPresented = true
        } else {
            dismiss()
        }
    }
}
