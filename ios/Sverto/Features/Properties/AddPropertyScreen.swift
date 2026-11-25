import SvertoCore
import SwiftUI

struct AddPropertyScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        AddPropertyForm(store: services.store)
    }
}

private struct AddPropertyForm: View {
    @State private var model: AddPropertyViewModel
    @State private var isCurrencyPickerPresented = false
    @State private var isConfirmingDiscard = false
    @State private var savedCount = 0
    @FocusState private var focusedField: Field?
    @Environment(ShellState.self) private var shell

    private enum Field {
        case name
        case value
        case purchasePrice
    }

    init(store: AppStore) {
        _model = State(initialValue: AddPropertyViewModel(store: store))
    }

    var body: some View {
        ScrollViewReader { proxy in
            form
                .onChange(of: model.saveError) { _, error in
                    guard let error else { return }
                    Haptics.error()
                    proxy.scrollTo("property-save-error", anchor: .top)
                    AccessibilityNotification.Announcement(error).post()
                }
        }
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle("Add property")
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden()
        .toolbar(.hidden, for: .tabBar)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel", action: cancel)
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
            ToolbarItemGroup(placement: .keyboard) {
                Spacer()
                Button("Done") { focusedField = nil }
            }
        }
        .confirmationDialog("Discard changes?", isPresented: $isConfirmingDiscard, titleVisibility: .visible) {
            Button("Discard changes", role: .destructive) { shell.pop() }
            Button("Keep editing", role: .cancel) {}
        }
        .sheet(isPresented: $isCurrencyPickerPresented) {
            CurrencyPickerSheet(title: "Currency", selectedId: model.currency?.id) { currency in
                model.currency = currency
            }
        }
        .sensoryFeedback(.success, trigger: savedCount)
        .task { await model.start() }
    }

    private var form: some View {
        Form {
            if let error = model.saveError {
                Section {
                    FormErrorRow(message: error)
                        .id("property-save-error")
                }
            }
            Section("Kind") {
                Picker("Kind", selection: $model.kind) {
                    Text(AccountType.realEstate.label).tag(PropertyKind.realEstate)
                    Text(AccountType.vehicle.label).tag(PropertyKind.vehicle)
                }
                .pickerStyle(.segmented)
                .sensoryFeedback(.selection, trigger: model.kind)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
            }
            Section("Name") {
                TextField("Name", text: $model.name, prompt: Text(model.kind == .vehicle ? "e.g. Golf GTI" : "e.g. Flat 12"))
                    .focused($focusedField, equals: .name)
                    .textInputAutocapitalization(.words)
                    .submitLabel(.next)
                    .onSubmit { focusedField = .purchasePrice }
                    .accessibilityLabel("Name")
            }
            Section {
                LabeledPickerField(
                    label: "Currency",
                    value: model.currency.map { "\($0.ticker) — \($0.name)" } ?? "Select currency",
                    isPlaceholder: model.currency == nil
                ) {
                    isCurrencyPickerPresented = true
                }
                DatePicker("Bought on", selection: $model.purchaseDate.animation(), in: ...Date(), displayedComponents: .date)
                LabeledContent("Purchase price") {
                    TextField("Purchase price", text: $model.purchasePriceText, prompt: Text("0.00"))
                        .keyboardType(.decimalPad)
                        .multilineTextAlignment(.trailing)
                        .focused($focusedField, equals: .purchasePrice)
                        .accessibilityLabel("Purchase price")
                }
            } header: {
                Text("Purchase")
            } footer: {
                Text("Sets the cost basis, so gains are measured from what you paid.")
            }
            if !model.isBoughtToday {
                Section {
                    LabeledContent("Value") {
                        TextField("Value today", text: $model.valueText, prompt: Text("0.00"))
                            .keyboardType(.decimalPad)
                            .multilineTextAlignment(.trailing)
                            .focused($focusedField, equals: .value)
                            .accessibilityLabel("Value today")
                    }
                } header: {
                    Text("Value today")
                } footer: {
                    Text("What it's worth today. The purchase price stays as its value on the day you bought it.")
                }
            }
            Section("Your ownership share") {
                OwnershipShareSlider(share: $model.ownershipShare)
            }
            loansSection
        }
        .disabled(model.isSaving)
    }

    private var loansSection: some View {
        Section {
            ForEach(model.loanCandidates, id: \.id) { loan in
                Button {
                    model.toggleLoan(loan.id)
                } label: {
                    LoanChoiceRow(loan: loan, isSelected: model.loanAccountIds.contains(loan.id))
                }
            }
        } header: {
            Text("Loans")
        } footer: {
            if let error = model.loanCandidatesError {
                Text("Couldn't load loan accounts. \(error)")
            } else if model.loanCandidates.isEmpty {
                Text("No mortgage or loan accounts to link. You can link one later from the property page.")
            } else {
                Text("Linked loans count towards the property's equity and leave your accounts list.")
            }
        }
    }

    private func cancel() {
        guard !model.isSaving else { return }
        focusedField = nil
        if model.hasChanges {
            isConfirmingDiscard = true
        } else {
            shell.pop()
        }
    }

    private func save() {
        focusedField = nil
        Task {
            if await model.save() {
                savedCount += 1
                shell.pop()
            }
        }
    }
}
