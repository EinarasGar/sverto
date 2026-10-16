import SvertoCore
import SwiftUI

struct AssetOverviewScreen: View {
    let assetId: Int32
    let userAsset: Bool

    @Environment(AppServices.self) private var services

    var body: some View {
        AssetOverviewContent(assetId: assetId, userAsset: userAsset, store: services.store)
    }
}

private struct AssetOverviewContent: View {
    let assetId: Int32
    let userAsset: Bool

    @State private var model: AssetOverviewViewModel
    @State private var isPairPickerPresented = false
    @State private var isAddRatePresented = false
    @State private var isAddPairPresented = false
    @State private var isDeleteConfirmationPresented = false
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    init(assetId: Int32, userAsset: Bool, store: AppStore) {
        self.assetId = assetId
        self.userAsset = userAsset
        _model = State(initialValue: AssetOverviewViewModel(store: store))
    }

    var body: some View {
        content
            .background(.screenBackground)
            .navigationTitle(model.title)
            .navigationSubtitle(model.subtitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if userAsset {
                    ToolbarItem(placement: .topBarTrailing) {
                        actionsMenu
                            .tint(.primary)
                    }
                }
            }
            .task {
                model.start()
                await model.load(assetId: assetId, userAsset: userAsset)
            }
            .onDisappear { model.stop() }
            .onChange(of: model.lastError) { _, event in
                if let event {
                    toasts.show(event.message)
                }
            }
            .sheet(isPresented: $isPairPickerPresented) {
                PairPickerSheet(pairs: model.pairs, selectedPairId: model.selectedPairId) { pair in
                    model.selectPair(pair.assetId)
                }
            }
            .sheet(isPresented: $isAddRatePresented) {
                if let referenceId = model.selectedPairId {
                    AddRateSheet { date, rate in
                        try await model.addRate(referenceId: referenceId, date: date, rate: rate)
                    }
                }
            }
            .sheet(isPresented: $isAddPairPresented) {
                AssetPickerSheet(title: "Add pair") { asset in
                    Task { await model.addPair(asset.id) }
                }
            }
            .confirmationDialog("Delete this asset?", isPresented: $isDeleteConfirmationPresented, titleVisibility: .visible) {
                Button("Delete asset", role: .destructive) {
                    Task { await deleteAsset() }
                }
            } message: {
                Text("Its pairs and rates are removed as well. This can't be undone.")
            }
    }

    @ViewBuilder
    private var content: some View {
        if model.isLoading {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let error = model.overview?.error {
            ContentUnavailableView("Couldn't load asset", systemImage: "exclamationmark.triangle", description: Text(error))
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    AssetChartSection(model: model) { isPairPickerPresented = true }
                    if let info = model.pairInfo, info.hasMetadata {
                        AssetInfoCard(info: info)
                    }
                    if let overview = model.overview {
                        AssetPositionSection(overview: overview, referenceTicker: model.referenceTicker, isReloading: model.isReloading)
                    }
                    Button {
                        let title = [model.title, model.subtitle].first { !$0.isEmpty } ?? "Transactions"
                        shell.push(.assetTransactions(assetId: assetId, accountId: nil, title: title, subtitle: nil))
                    } label: {
                        Label("View transactions", systemImage: "list.bullet.rectangle.portrait")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .padding(.top, 8)
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
    }

    private var actionsMenu: some View {
        Menu {
            Button("Add rate", systemImage: "plus.forwardslash.minus") { isAddRatePresented = true }
                .disabled(model.selectedPairId == nil || model.isLoading)
            Button("Add pair", systemImage: "arrow.left.arrow.right") { isAddPairPresented = true }
            Divider()
            Button("Delete asset", systemImage: "trash", role: .destructive) { isDeleteConfirmationPresented = true }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .accessibilityLabel("Actions")
    }

    private func deleteAsset() async {
        if await model.delete() {
            shell.pop()
        }
    }
}
