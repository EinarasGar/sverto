import SvertoCore
import SwiftUI

struct BindingAccountPickerSheet: View {
    let accounts: [AccountListItem]
    let onSelect: (AccountListItem) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Group {
                if accounts.isEmpty {
                    ContentUnavailableView("No accounts available", systemImage: "building.columns")
                } else {
                    List {
                        Section("Write into") {
                            ForEach(accounts, id: \.id) { account in
                                Button {
                                    onSelect(account)
                                    dismiss()
                                } label: {
                                    Text(account.name)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                }
                                .foregroundStyle(.primary)
                            }
                        }
                    }
                    .listStyle(.insetGrouped)
                }
            }
            .navigationTitle("Choose account")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
