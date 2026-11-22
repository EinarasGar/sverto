import SvertoCore
import SwiftUI

struct LinkLoanSheet: View {
    let onLink: (_ accountId: String) async throws -> Void

    @Environment(AppServices.self) private var services
    @Environment(\.dismiss) private var dismiss
    @State private var candidates: [AccountListItem] = []
    @State private var isLoading = true
    @State private var loadError: String?
    @State private var linkingAccountId: String?
    @State private var submitError: String?

    var body: some View {
        NavigationStack {
            Group {
                if isLoading {
                    ProgressView()
                        .controlSize(.large)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let loadError {
                    ContentUnavailableView {
                        Label("Couldn't load loans", systemImage: "exclamationmark.triangle")
                    } description: {
                        Text(loadError)
                    } actions: {
                        Button("Retry") {
                            Task { await load() }
                        }
                        .buttonStyle(.bordered)
                    }
                } else if candidates.isEmpty {
                    ContentUnavailableView(
                        "No loans to link",
                        systemImage: "link",
                        description: Text("Mortgage and loan accounts that aren't linked to a property show up here.")
                    )
                } else {
                    List {
                        if let submitError {
                            Section {
                                FormErrorRow(message: submitError)
                            }
                        }
                        Section {
                            ForEach(candidates, id: \.id) { loan in
                                Button {
                                    link(loan.id)
                                } label: {
                                    LoanChoiceRow(loan: loan, isSelected: false, isBusy: linkingAccountId == loan.id)
                                }
                            }
                        } footer: {
                            Text("The loan's balance counts towards this property's equity, and the loan leaves your accounts list.")
                        }
                    }
                    .listStyle(.insetGrouped)
                    .disabled(linkingAccountId != nil)
                }
            }
            .navigationTitle("Link loan")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .tint(.primary)
                        .disabled(linkingAccountId != nil)
                }
            }
        }
        .presentationDetents([.medium, .large])
        .interactiveDismissDisabled(linkingAccountId != nil)
        .task { await load() }
    }

    private func load() async {
        isLoading = true
        loadError = nil
        do {
            candidates = try await services.store.getLoanCandidates()
        } catch {
            loadError = error.friendlyMessage
        }
        isLoading = false
    }

    private func link(_ accountId: String) {
        guard linkingAccountId == nil else { return }
        linkingAccountId = accountId
        submitError = nil
        Task {
            do {
                try await onLink(accountId)
                dismiss()
            } catch {
                submitError = error.friendlyMessage
                linkingAccountId = nil
            }
        }
    }
}

struct LoanChoiceRow: View {
    let loan: AccountListItem
    let isSelected: Bool
    var isBusy = false

    var body: some View {
        HStack(spacing: Layout.rowSpacing) {
            AccountTypeBadge(typeId: loan.accountTypeId, size: 36)
            VStack(alignment: .leading, spacing: 2) {
                Text(loan.name)
                    .foregroundStyle(Color.primary)
                    .fixedSize(horizontal: false, vertical: true)
                Text(AccountTypeVisuals.label(loan.accountTypeId))
                    .font(.subheadline)
                    .foregroundStyle(Color.secondary)
            }
            Spacer(minLength: 8)
            if isBusy {
                ProgressView()
            } else if isSelected {
                Image(systemName: "checkmark")
                    .fontWeight(.semibold)
                    .foregroundStyle(.tint)
            }
        }
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
