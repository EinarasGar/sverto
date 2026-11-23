import SvertoCore
import SwiftUI

struct BindingSetupScreen: View {
    let connectionId: String
    @Environment(AppServices.self) private var services

    var body: some View {
        BindingSetupContent(store: services.store, connectionId: connectionId)
    }
}

private struct LinkTarget: Identifiable {
    let account: ProviderAccount
    var id: String { account.providerAccountId }
}

private struct BindingSetupContent: View {
    @State private var model: BindingSetupViewModel
    @State private var linkTarget: LinkTarget?
    @Environment(ShellState.self) private var shell
    private let connectionId: String

    init(store: AppStore, connectionId: String) {
        _model = State(initialValue: BindingSetupViewModel(store: store, connectionId: connectionId))
        self.connectionId = connectionId
    }

    private var actionTitle: String {
        let verb = model.linksByImport ? "Import" : "Link"
        return switch model.choices.count {
        case 0: "Done"
        case 1: "\(verb) 1 account"
        case let count: "\(verb) \(count) accounts"
        }
    }

    private var startExplanation: String {
        switch model.importStart {
        case .lastEntry:
            "Starts on the day of the newest transaction already in each Sverto account. Accounts with none get all available history."
        case .all: "All the history available for each account."
        case .date: "Transactions before this date are skipped."
        }
    }

    var body: some View {
        List {
            Section {
                Text("Choose which Sverto account each imported account writes into. You can skip any and add them later.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .listRowBackground(Color.clear)
            }
            Section("Accounts") {
                if let error = model.loadError {
                    FormErrorRow(message: error)
                }
                if model.loading, model.providerAccounts.isEmpty {
                    BindingRowsSkeleton()
                } else if model.providerAccounts.isEmpty, model.loadError == nil {
                    Text("No provider accounts available.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(model.providerAccounts, id: \.providerAccountId) { account in
                        ProviderAccountRow(
                            account: account,
                            binding: model.bindings[account.providerAccountId],
                            choice: model.choices[account.providerAccountId],
                            enabled: !model.linking,
                            onOpen: { open(account) },
                            onOpenImport: { shell.push(.importSession(id: $0)) },
                            onLink: { linkTarget = LinkTarget(account: account) },
                            onClearChoice: { model.clearChoice(providerAccountId: account.providerAccountId) }
                        )
                    }
                }
            }
            if model.hasUnlinkedAccounts {
                Section {
                    Picker("Import transactions from", selection: $model.importStart.animation()) {
                        ForEach(ImportStart.allCases, id: \.self) { option in
                            Text(option.label).tag(option)
                        }
                    }
                    .pickerStyle(.segmented)
                    if model.importStart == .date {
                        DatePicker("From", selection: $model.startDate, in: ...Date(), displayedComponents: .date)
                    }
                } header: {
                    Text("Import transactions from")
                } footer: {
                    Text(startExplanation)
                }
                .disabled(model.linking)
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Link accounts")
        .navigationBarTitleDisplayMode(.inline)
        .safeAreaInset(edge: .bottom) {
            ConnectorHeroButton(title: actionTitle, isBusy: model.linking) {
                if model.choices.isEmpty {
                    shell.pop()
                } else {
                    link()
                }
            }
            .disabled(model.linking)
        }
        .onAppear { model.resume() }
        .onDisappear { model.pause() }
        .refreshable { await model.load() }
        .onChange(of: shell.pendingBindingChoice, initial: true) { _, choice in
            guard let choice, choice.connectionId == connectionId else { return }
            model.choose(providerAccountId: choice.providerAccountId, account: choice.account)
            shell.pendingBindingChoice = nil
        }
        .toastOnError(model.error)
        .sheet(item: $linkTarget) { target in
            BindingAccountPickerSheet(accounts: model.svertoAccounts) { sverto in
                model.choose(providerAccountId: target.account.providerAccountId, account: sverto)
            }
        }
    }

    private func open(_ account: ProviderAccount) {
        shell.push(.providerAccountDetail(
            connectionId: connectionId,
            providerAccountId: account.providerAccountId,
            displayName: account.displayName,
            currency: account.currency,
            fromLinkSetup: true
        ))
    }

    private func link() {
        let owner = shell.navigationOwner
        Task {
            if let sessionId = await model.linkAccounts() {
                shell.openImport(sessionId: sessionId, replacingLinkSetupIn: owner)
            }
        }
    }
}

private struct ProviderAccountRow: View {
    let account: ProviderAccount
    let binding: ConnectorBinding?
    let choice: AccountListItem?
    let enabled: Bool
    let onOpen: () -> Void
    let onOpenImport: (String) -> Void
    let onLink: () -> Void
    let onClearChoice: () -> Void
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var name: String { ConnectorNames.humanizeAccountName(account.displayName) }

    private var details: String? {
        let parts = [
            account.currency,
            account.accountNumberLast4.map { "••\($0)" },
            account.balance.map { Money.format($0.amount, ticker: $0.currency) },
        ].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    private var isChosen: Bool { binding == nil && choice != nil }

    private var history: String {
        account.history.label.map { " · \($0)" } ?? ""
    }

    private var link: String {
        switch (binding, choice) {
        case let (binding?, _) where binding.isWaitingForImport: "Being imported into \(binding.svertoAccountName)"
        case let (binding?, _): "Linked to \(binding.svertoAccountName)"
        case let (nil, choice?): "→ \(choice.name)"
        case (nil, nil): "Not linked"
        }
    }

    private var spokenLink: String {
        if isChosen, let choice { "Will write into \(choice.name)" } else { link }
    }

    var body: some View {
        let layout = dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(spacing: 12))
        layout {
            Button(action: onOpen) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(name)
                    if let details {
                        Text(details)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                    Text("\(Text(link).foregroundStyle(isChosen ? Color.brand : .secondary))\(Text(history).foregroundStyle(.secondary))")
                        .font(.footnote)
                        .accessibilityLabel("\(spokenLink)\(history)")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
                .contentShape(.rect)
            }
            .buttonStyle(.borderless)
            .foregroundStyle(.primary)
            .accessibilityHint("Shows this account's transactions")
            trailing
        }
    }

    @ViewBuilder
    private var trailing: some View {
        if let binding {
            if binding.isWaitingForImport {
                if let sessionId = binding.importSessionId {
                    Button("Open the import") { onOpenImport(sessionId) }
                        .buttonStyle(.borderless)
                        .frame(minHeight: 44)
                        .accessibilityLabel("Open the import of \(name)")
                }
            } else {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(.tint)
                    .accessibilityLabel("Linked")
            }
        } else if choice != nil {
            Button(action: onClearChoice) {
                Image(systemName: "xmark.circle.fill")
                    .font(.title3)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(.secondary)
                    .frame(minWidth: 44, minHeight: 44)
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .disabled(!enabled)
            .accessibilityLabel("Don't link \(name)")
        } else {
            Button(action: onLink) {
                Text("Link")
                    .frame(minHeight: 44)
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .disabled(!enabled)
            .accessibilityLabel("Link \(name)")
        }
    }
}
