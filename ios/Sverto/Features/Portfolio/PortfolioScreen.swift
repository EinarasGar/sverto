import SwiftUI
import SvertoCore

struct PortfolioScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        PortfolioRoot(store: services.store)
    }
}

private struct PortfolioRoot: View {
    @State private var portfolio: HomeViewModel
    @State private var markets: MarketsListViewModel
    @State private var query = ""
    @State private var isSearchPresented = false

    init(store: AppStore) {
        _portfolio = State(initialValue: HomeViewModel(store: store))
        _markets = State(initialValue: MarketsListViewModel(store: store))
    }

    var body: some View {
        PortfolioSearchSwitch(portfolio: portfolio, markets: markets, isSearching: isSearchPresented)
            .navigationTitle("Portfolio")
            .navigationBarTitleDisplayMode(.large)
            .modifier(ToolbarSearch(text: $query, isPresented: $isSearchPresented, prompt: "Search assets"))
            .toolbar {
                if !isSearchPresented {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Search assets", systemImage: "magnifyingglass") {
                            isSearchPresented = true
                        }
                        .labelStyle(.iconOnly)
                        .tint(.primary)
                    }
                    ProfileToolbar()
                }
                ToolbarItem(placement: .topBarLeading) {
                    Image("Logo")
                        .resizable()
                        .scaledToFit()
                        .frame(height: 24)
                        .foregroundStyle(.tint)
                        .accessibilityLabel("Sverto")
                }
                .sharedBackgroundVisibility(.hidden)
            }
            .onChange(of: query) { _, newValue in
                markets.onQueryChange(newValue)
            }
            .task { await portfolio.start() }
            .onDisappear { portfolio.stop() }
    }
}

private struct PortfolioSearchSwitch: View {
    let portfolio: HomeViewModel
    let markets: MarketsListViewModel
    let isSearching: Bool

    @Environment(ShellState.self) private var shell

    var body: some View {
        Group {
            if isSearching {
                AssetSearchResults(model: markets) { asset in
                    shell.push(.assetOverview(assetId: asset.id, userAsset: false))
                }
            } else {
                PortfolioContent(model: portfolio, onSelectHolding: selectHolding) { liability in
                    shell.push(.accountDetail(
                        accountId: liability.accountId,
                        accountName: liability.accountName,
                        accountTypeId: liability.accountTypeId
                    ))
                }
            }
        }
        .onChange(of: isSearching, initial: true) { _, searching in
            if searching {
                markets.onSearchExpanded()
            }
        }
    }

    private func selectHolding(_ holding: HoldingItem) {
        if let property = holding.property {
            shell.push(.propertyDetail(propertyId: property.propertyId, name: holding.assetName))
        } else if holding.assetTypeId == HoldingAssetType.currency.rawValue {
            shell.push(.currencyDetail(assetId: holding.assetId, name: holding.assetName))
        } else {
            shell.push(.assetOverview(assetId: holding.assetId, userAsset: false))
        }
    }
}
