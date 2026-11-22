import SvertoCore
import SwiftUI

struct ValuationSheet: View {
    let title: String
    let currencyTicker: String
    let onSubmit: (_ value: Double, _ date: Date) async throws -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var valueText: String
    @State private var date: Date
    @State private var isSaving = false
    @State private var submitError: String?
    @FocusState private var isValueFocused: Bool

    init(
        title: String,
        currencyTicker: String,
        valuation: ChartPoint? = nil,
        onSubmit: @escaping (_ value: Double, _ date: Date) async throws -> Void
    ) {
        self.title = title
        self.currencyTicker = currencyTicker
        self.onSubmit = onSubmit
        _valueText = State(initialValue: valuation.map { Self.editableText($0.value) } ?? "")
        _date = State(initialValue: valuation.map { DateText.date(fromUnix: $0.timestamp) } ?? Date())
    }

    private var value: Double? { RateInput.parse(valueText) }
    private var showsError: Bool { !valueText.trimmingCharacters(in: .whitespaces).isEmpty && value == nil }

    var body: some View {
        NavigationStack {
            Form {
                if let submitError {
                    Section("Couldn't save valuation") {
                        FormErrorRow(message: submitError)
                    }
                }
                Section {
                    TextField("Value", text: $valueText, prompt: Text("0.00"))
                        .keyboardType(.decimalPad)
                        .focused($isValueFocused)
                        .accessibilityLabel("Value in \(currencyTicker)")
                    DatePicker("Date", selection: $date, in: ...Date(), displayedComponents: .date)
                } header: {
                    Text("Value (\(currencyTicker))")
                } footer: {
                    if showsError {
                        Text("Enter a positive number")
                            .foregroundStyle(.red)
                    }
                }
            }
            .disabled(isSaving)
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
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
                    .disabled(value == nil || isSaving)
                    .accessibilityLabel(isSaving ? "Saving valuation" : "Save valuation")
                }
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button("Done") { isValueFocused = false }
                }
            }
            .onAppear { isValueFocused = true }
            .onChange(of: submitError) { _, message in
                if let message { AccessibilityNotification.Announcement(message).post() }
            }
        }
        .presentationDetents(dynamicTypeSize.isAccessibilitySize ? [.large] : [.medium, .large])
        .interactiveDismissDisabled(isSaving)
    }

    private func save() {
        guard !isSaving, let value else { return }
        isValueFocused = false
        isSaving = true
        submitError = nil
        Task {
            do {
                try await onSubmit(value, date)
                dismiss()
            } catch {
                submitError = error.friendlyMessage
                isSaving = false
            }
        }
    }

    private static func editableText(_ value: Double) -> String {
        value.formatted(.number.grouping(.never).precision(.fractionLength(0...2)).locale(Locale(identifier: "en_US_POSIX")))
    }
}
