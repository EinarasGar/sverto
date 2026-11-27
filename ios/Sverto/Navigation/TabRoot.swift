import SwiftUI

struct TabRoot: View {
    @Environment(AppServices.self) private var services
    @Environment(ShellState.self) private var shell
    @Environment(\.scenePhase) private var scenePhase
    @State private var toasts = ToastCenter()

    var body: some View {
        @Bindable var shell = shell
        TransactionsModelsHost(store: services.store) {
            TabView(selection: tabSelection) {
                Tab(AppTab.portfolio.title, systemImage: AppTab.portfolio.systemImage, value: AppTab.portfolio) {
                    tabStack(path: $shell.portfolioPath) { PortfolioScreen() }
                }
                Tab(AppTab.transactions.title, systemImage: AppTab.transactions.systemImage, value: AppTab.transactions) {
                    TransactionsScreen()
                }
                Tab(AppTab.accounts.title, systemImage: AppTab.accounts.systemImage, value: AppTab.accounts) {
                    AccountsScreen()
                }
                Tab(AppTab.chat.title, systemImage: AppTab.chat.systemImage, value: AppTab.chat) {
                    tabStack(path: $shell.chatPath) { AiChatScreen() }
                }
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        .settingsSheet()
        .handlesSvertoDeepLinks()
        .environment(toasts)
        .focusedSceneValue(\.shell, shell)
        .task { services.connection.start() }
        .onDisappear { services.connection.stop() }
        .onChange(of: scenePhase, initial: true) { _, phase in
            if phase == .active {
                Task { await drainSharedFiles() }
            }
        }
    }

    private func drainSharedFiles() async {
        let files = await SharedFileInbox.drain()
        guard !files.isEmpty else { return }
        shell.pendingSharedFiles = files
        shell.selectedTab = .transactions
    }

    private var tabSelection: Binding<AppTab> {
        Binding(get: { shell.selectedTab }, set: { shell.select($0) })
    }

    private func tabStack<Root: View>(path: Binding<[AppRoute]>, @ViewBuilder root: () -> Root) -> some View {
        NavigationStack(path: path) {
            root()
                .offlineBannerInset()
                .navigationDestination(for: AppRoute.self) { RouteDestination(route: $0) }
        }
        .toastHost(toasts)
    }
}

extension View {
    func offlineBannerInset() -> some View {
        safeAreaInset(edge: .top, spacing: 0) { OfflineBanner() }
    }
}
