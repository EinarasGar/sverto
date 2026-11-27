import SvertoCore
import SwiftUI

struct BindingRows<RemoveDialog: ViewModifier>: View {
    let binding: ConnectorBinding
    let rawName: String
    @Binding var isExpanded: Bool
    let onOpenTransactions: () -> Void
    let onOpenImport: (String) -> Void
    let onUpdate: (BindingWriteMode, BindingStatus) -> Void
    let onRemove: () -> Void
    let removeDialog: RemoveDialog

    private var isPaused: Bool { binding.isPaused }

    private var syncLine: String {
        if binding.isWaitingForImport { return "Being imported" }
        guard let lastSyncAt = binding.lastSyncAt else { return "Never synced" }
        let status = binding.lastSyncStatus.map { " · \($0)" } ?? ""
        return "Last synced \(ConnectorTimeFormat.relative(lastSyncAt))\(status)"
    }

    private var isTrusted: Binding<Bool> {
        Binding(
            get: { binding.writeMode == .trusted },
            set: { onUpdate($0 ? .trusted : .ghost, isPaused ? .paused : .active) }
        )
    }

    private var isEnabled: Binding<Bool> {
        Binding(
            get: { !isPaused },
            set: { onUpdate(binding.writeMode, $0 ? .active : .paused) }
        )
    }

    private var importNote: some View {
        Text("Finish or delete its import to change or remove this account.")
            .font(.footnote)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    var body: some View {
        DisclosureGroup(isExpanded: $isExpanded) {
            Button(action: onOpenTransactions) {
                HStack {
                    Text("View transactions")
                    Spacer()
                    Image(systemName: "chevron.forward")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.tertiary)
                        .accessibilityHidden(true)
                }
            }
            .foregroundStyle(.primary)
            if binding.isWaitingForImport {
                if let sessionId = binding.importSessionId {
                    Button { onOpenImport(sessionId) } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Open the import")
                                importNote
                            }
                            Spacer()
                            Image(systemName: "chevron.forward")
                                .font(.footnote.weight(.semibold))
                                .foregroundStyle(.tertiary)
                                .accessibilityHidden(true)
                        }
                    }
                    .foregroundStyle(.primary)
                } else {
                    importNote
                }
            } else {
                Toggle(isOn: isTrusted) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Trusted writes")
                        Text("Off: imports arrive as ghosts pending your review. On: imports are normal transactions.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
                Toggle(isOn: isEnabled) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Enabled")
                        Text("Paused bindings are skipped when syncing.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
                Button(role: .destructive, action: onRemove) {
                    Label {
                        Text("Remove linked account")
                            .foregroundStyle(Color.loss)
                    } icon: {
                        Image(systemName: "trash")
                            .foregroundStyle(Color.loss)
                    }
                }
                .modifier(removeDialog)
            }
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(ConnectorNames.humanizeAccountName(rawName))
                Text("Writes into \(binding.svertoAccountName)")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Text(binding.lastSyncError ?? syncLine)
                    .font(.footnote)
                    .foregroundStyle(binding.lastSyncError == nil ? Color.secondary : Color.loss)
            }
            .fixedSize(horizontal: false, vertical: true)
        }
    }
}
