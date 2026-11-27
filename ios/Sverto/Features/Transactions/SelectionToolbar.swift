import SwiftUI

struct SelectionToolbar<DeleteDialog: ViewModifier>: ToolbarContent {
    let count: Int
    let isBusy: Bool
    let showMarkReviewed: Bool
    let showGroup: Bool
    let onClose: () -> Void
    let onMarkReviewed: () -> Void
    let onGroup: () -> Void
    let onDelete: () -> Void
    let deleteDialog: DeleteDialog

    var body: some ToolbarContent {
        ToolbarItem(placement: .cancellationAction) {
            Button("Close", action: onClose)
                .tint(.primary)
                .disabled(isBusy)
                .accessibilityLabel("Exit selection")
        }
        ToolbarItem(placement: .principal) {
            HStack(spacing: 8) {
                if isBusy {
                    ProgressView()
                        .accessibilityLabel("Updating selection")
                }
                Text("\(count) selected")
                    .font(.headline)
                    .monospacedDigit()
            }
        }
        ToolbarItemGroup(placement: .bottomBar) {
            if showMarkReviewed {
                Button(action: onMarkReviewed) {
                    Label("Mark reviewed", systemImage: "checkmark")
                        .frame(minWidth: 44, minHeight: 44)
                }
                .disabled(isBusy)
                Spacer()
            }
            if showGroup {
                Button(action: onGroup) {
                    Label("Group", systemImage: "square.3.layers.3d")
                        .frame(minWidth: 44, minHeight: 44)
                }
                .tint(.primary)
                .disabled(isBusy)
                .accessibilityLabel("Group transactions")
                Spacer()
            }
            Button(role: .destructive, action: onDelete) {
                Label("Delete", systemImage: "trash")
                    .frame(minWidth: 44, minHeight: 44)
            }
            .tint(.red)
            .disabled(isBusy)
            .modifier(deleteDialog)
        }
    }
}
