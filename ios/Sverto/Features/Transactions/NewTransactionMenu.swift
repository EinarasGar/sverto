import SwiftUI

struct NewTransactionMenu: View {
    let onAddReceiptPhoto: () -> Void
    let onManualEntry: () -> Void
    let onNewGroup: () -> Void
    let onImportStatements: () -> Void

    var body: some View {
        Menu {
            Button("Add receipt photo", systemImage: "photo.badge.plus", action: onAddReceiptPhoto)
            Button("Manual entry", systemImage: "square.and.pencil", action: onManualEntry)
            Button("New group", systemImage: "square.3.layers.3d", action: onNewGroup)
            Section {
                Button("Import statements", systemImage: "arrow.up.doc", action: onImportStatements)
            }
        } label: {
            Image(systemName: "plus")
        }
        .tint(.primary)
        .accessibilityLabel("Add transaction")
    }
}
