import SvertoCore
import SwiftUI

struct ConnectionDetailScreen: View {
    let connectionId: String
    @Environment(AppServices.self) private var services

    var body: some View {
        ConnectionDetailContent(store: services.store, connectionId: connectionId)
    }
}

private struct ConnectionDetailContent: View {
    @State private var model: ConnectionDetailViewModel
    @State private var expandedBindingId: String?
    @State private var bindingToRemove: ConnectorBinding?
    @State private var isConfirmingRevoke = false
    @Environment(ShellState.self) private var shell
    private let connectionId: String

    init(store: AppStore, connectionId: String) {
        _model = State(initialValue: ConnectionDetailViewModel(store: store, connectionId: connectionId))
        self.connectionId = connectionId
    }

    private var connector: ConnectorInfo? {
        model.connection?.kind?.info
    }

    private var isRemovingBinding: Binding<Bool> {
        Binding(get: { bindingToRemove != nil }, set: { if !$0 { bindingToRemove = nil } })
    }

    private func isExpanded(_ binding: ConnectorBinding) -> Binding<Bool> {
        Binding(
            get: { expandedBindingId == binding.id },
            set: { expandedBindingId = $0 ? binding.id : nil }
        )
    }

    var body: some View {
        List {
            if model.connection == nil, model.loading {
                Section { ConnectionHeaderSkeleton() }
                Section("Linked accounts") { BindingRowsSkeleton(rows: 2) }
            } else {
                if let connection = model.connection {
                    Section {
                        ConnectionHeaderRow(connection: connection, connector: connector)
                    }
                }
                linkedAccountsSection
                Section {
                    Button("Revoke connection", role: .destructive) {
                        isConfirmingRevoke = true
                    }
                    .frame(maxWidth: .infinity)
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(connector?.name ?? "Connection")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { Task { await model.load() } }
        .refreshable { await model.load() }
        .toastOnError(model.error)
        .confirmationDialog(
            "Remove linked account",
            isPresented: isRemovingBinding,
            titleVisibility: .visible,
            presenting: bindingToRemove
        ) { binding in
            Button("Remove", role: .destructive) {
                Task { await model.deleteBinding(id: binding.id) }
            }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("Are you sure you want to remove this linked account? This cannot be undone.")
        }
        .confirmationDialog("Revoke connection", isPresented: $isConfirmingRevoke, titleVisibility: .visible) {
            Button("Revoke", role: .destructive) {
                let owner = shell.navigationOwner
                Task {
                    if await model.revoke() {
                        shell.popToConnectors(in: owner)
                    }
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Are you sure you want to revoke this connection? Linked accounts will stop syncing.")
        }
    }

    private var linkedAccountsSection: some View {
        Section {
            if !model.bindings.isEmpty {
                ForEach(model.bindings, id: \.id) { binding in
                    BindingRows(
                        binding: binding,
                        rawName: model.rawAccountName(for: binding),
                        isExpanded: isExpanded(binding),
                        onOpenTransactions: {
                            shell.push(.providerAccountDetail(
                                connectionId: connectionId,
                                providerAccountId: binding.providerAccountId,
                                displayName: model.rawAccountName(for: binding),
                                currency: nil
                            ))
                        },
                        onOpenImport: { shell.push(.importSession(id: $0)) },
                        onUpdate: { writeMode, status in
                            Task { await model.updateBinding(binding, writeMode: writeMode, status: status) }
                        },
                        onRemove: { bindingToRemove = binding }
                    )
                }
            } else if model.loading {
                BindingRowSkeleton()
            } else {
                EmptyState(
                    title: "No accounts linked yet",
                    systemImage: "link",
                    description: "Link an account so imported transactions know where to go."
                )
                .listRowBackground(Color.clear)
            }
        } header: {
            HStack {
                Text("Linked accounts")
                Spacer()
                Button("Add") {
                    shell.push(.bindingSetup(connectionId: connectionId))
                }
                .font(.subheadline)
            }
            .textCase(nil)
        }
    }
}

private struct ConnectionHeaderRow: View {
    let connection: ConnectorConnection
    let connector: ConnectorInfo?
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var supporting: String? {
        connection.kind?.isBank == true ? connection.consentLabel : connection.keyLabel
    }

    var body: some View {
        HStack(spacing: 16) {
            ProviderAvatar(systemImage: connector?.systemImage ?? ConnectorCatalog.fallbackSystemImage)
            statusLayout {
                VStack(alignment: .leading, spacing: 2) {
                    Text(connector?.name ?? connection.providerKind.capitalized)
                        .font(.headline)
                    if let supporting {
                        Text(supporting)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                }
                if !dynamicTypeSize.isAccessibilitySize {
                    Spacer(minLength: 8)
                }
                ConnectionStatusChip(status: connection.connectionStatus)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.vertical, 4)
    }

    private var statusLayout: AnyLayout {
        dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(spacing: 8))
    }
}
