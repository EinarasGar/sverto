import SvertoCore
import SwiftUI

struct CustomAssetsScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        CustomAssetsList(store: services.store)
    }
}

private struct CustomAssetsList: View {
    @State private var model: CustomAssetsViewModel
    @State private var isCreatePresented = false
    @State private var assetPendingDeletion: AssetSummary?
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    init(store: AppStore) {
        _model = State(initialValue: CustomAssetsViewModel(store: store))
    }

    var body: some View {
        content
            .background(.screenBackground)
            .navigationTitle("Custom Assets")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        isCreatePresented = true
                    } label: {
                        Image(systemName: "plus")
                    }
                    .tint(.primary)
                    .accessibilityLabel("Add asset")
                    .disabled(model.isLoading || model.assetTypes.isEmpty)
                }
            }
            .task { await model.load() }
            .onChange(of: model.lastError) { _, event in
                if let event {
                    toasts.showError(event.message)
                }
            }
            .sheet(isPresented: $isCreatePresented) {
                CreateAssetSheet(assetTypes: model.assetTypes) { name, ticker, assetType, baseAssetId in
                    try await model.createAsset(name: name, ticker: ticker, assetType: assetType, baseAssetId: baseAssetId)
                }
            }
            .confirmationDialog(
                "Delete this asset?",
                isPresented: Binding(get: { assetPendingDeletion != nil }, set: { if !$0 { assetPendingDeletion = nil } }),
                titleVisibility: .visible,
                presenting: assetPendingDeletion
            ) { asset in
                Button("Delete \(asset.ticker.isEmpty ? asset.name : asset.ticker)", role: .destructive) {
                    Task { await model.deleteAsset(id: asset.id) }
                }
            } message: { _ in
                Text("Its pairs and rates are removed as well. This can't be undone.")
            }
    }

    @ViewBuilder
    private var content: some View {
        if model.isLoading {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let error = model.loadError, model.assets.isEmpty {
            ContentUnavailableView {
                Label("Couldn't load assets", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") { Task { await model.load() } }
            }
        } else if model.assets.isEmpty {
            ContentUnavailableView("No custom assets yet", systemImage: "chart.pie", description: Text("Tap + to create your first asset."))
        } else {
            List(model.assets, id: \.id) { asset in
                Button {
                    shell.push(.assetOverview(assetId: asset.id, userAsset: true))
                } label: {
                    AssetListRow(ticker: asset.ticker, name: asset.name, supportingText: supportingText(for: asset), showsDisclosure: true)
                }
                .accessibilityHint("Shows asset details")
                .swipeActions(edge: .trailing) {
                    Button("Delete", systemImage: "trash", role: .destructive) {
                        assetPendingDeletion = asset
                    }
                }
            }
            .listStyle(.insetGrouped)
        }
    }

    private func supportingText(for asset: AssetSummary) -> String {
        let type = asset.assetType.trimmingCharacters(in: .whitespaces)
        return type.isEmpty ? asset.name : "\(asset.name) · \(type)"
    }
}
