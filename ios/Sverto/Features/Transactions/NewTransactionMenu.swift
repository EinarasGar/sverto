import SwiftUI

struct NewTransactionMenu: View {
    let onAddReceipt: (QuickUploadOrigin) -> Void
    let onManualEntry: () -> Void
    let onNewGroup: () -> Void
    let onImportStatements: () -> Void

    var body: some View {
        Menu {
            Menu("Add receipt", systemImage: "receipt") {
                Button("Photos", systemImage: "photo.on.rectangle") { onAddReceipt(.photos) }
                if CameraPicker.isAvailable {
                    Button("Camera", systemImage: "camera") { onAddReceipt(.camera) }
                }
                Button("Files", systemImage: "folder") { onAddReceipt(.files) }
            }
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
