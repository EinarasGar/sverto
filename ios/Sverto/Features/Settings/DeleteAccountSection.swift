import SwiftUI

struct DeleteAccountSection: View {
    @Environment(SessionCoordinator.self) private var session
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts
    @State private var isConfirming = false
    @State private var isDeleting = false

    private static let confirmationBody =
        "This deletes your Sverto account and its financial records, custom assets, categories, chats and connections. You will be signed out. We also request deletion of uploaded files and sign-in data; logs and backups follow the privacy policy. Shared assets, exchange rates and seeded categories remain. This cannot be undone."

    var body: some View {
        Section {
            Button(role: .destructive) {
                isConfirming = true
            } label: {
                HStack(spacing: 12) {
                    SettingsIcon(systemImage: "trash", tint: .red)
                    Text(isDeleting ? "Deleting…" : "Delete account")
                    if isDeleting {
                        Spacer()
                        ProgressView()
                    }
                }
            }
            .disabled(isDeleting)
            .alert("Delete account", isPresented: $isConfirming) {
                Button("Delete account", role: .destructive) {
                    Task { await deleteAccount() }
                }
                Button("Keep account", role: .cancel) {}
            } message: {
                Text(Self.confirmationBody)
            }
        }
    }

    private func deleteAccount() async {
        isDeleting = true
        defer { isDeleting = false }
        if let error = await session.deleteUserAndSignOut() {
            toasts.showError(error.friendlyMessage)
            return
        }
        shell.dismissSettings()
    }
}
