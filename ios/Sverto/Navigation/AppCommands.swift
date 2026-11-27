import SwiftUI

struct AppCommands: Commands {
    @FocusedValue(\.shell) private var shell

    private var isUnavailable: Bool {
        shell == nil || shell?.isSettingsPresented == true
    }

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("New Transaction") {
                guard let shell else { return }
                shell.selectedTab = .transactions
                shell.push(.createTransaction(typeKey: TransactionTypeCatalog.regular.key, quickUploadId: nil))
            }
            .keyboardShortcut("n")
            .disabled(isUnavailable || shell?.transactionsPath.last?.takesFocus == true)
        }
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { shell?.presentSettings() }
                .keyboardShortcut(",")
                .disabled(isUnavailable)
        }
        CommandGroup(before: .sidebar) {
            Button("Refresh") { shell?.refreshRequests += 1 }
                .keyboardShortcut("r")
                .disabled(isUnavailable || shell?.selectedTab == .chat)
            Divider()
            ForEach(Array(AppTab.allCases.enumerated()), id: \.element) { index, tab in
                Button(tab.title) { shell?.selectedTab = tab }
                    .keyboardShortcut(KeyEquivalent(Character("\(index + 1)")))
                    .disabled(isUnavailable)
            }
            Divider()
        }
    }
}

extension FocusedValues {
    @Entry var shell: ShellState?
}
