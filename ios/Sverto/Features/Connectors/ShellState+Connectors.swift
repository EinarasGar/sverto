import Foundation

enum NavigationOwner {
    case tab(AppTab)
    case settings
}

extension ShellState {
    var navigationOwner: NavigationOwner {
        isSettingsPresented ? .settings : .tab(selectedTab)
    }

    func pop(in owner: NavigationOwner) {
        updatePath(in: owner) { path in
            if !path.isEmpty { path.removeLast() }
        }
    }

    func finishConnecting(connectionId: String, in owner: NavigationOwner) {
        updatePath(in: owner) { path in
            if let last = path.last, Self.isConnectRoute(last) {
                path.removeLast()
            }
            path.append(.connectionDetail(connectionId: connectionId))
            path.append(.bindingSetup(connectionId: connectionId))
        }
    }

    func openImport(sessionId: String, replacingLinkSetupIn owner: NavigationOwner) {
        updatePath(in: owner) { path in
            if case .bindingSetup = path.last {
                path.removeLast()
            }
            path.append(.importSession(id: sessionId))
        }
    }

    func popToConnectors(in owner: NavigationOwner) {
        updatePath(in: owner) { path in
            guard let index = path.lastIndex(of: .connectors) else { return }
            path.removeSubrange(path.index(after: index)...)
        }
    }

    private func updatePath(in owner: NavigationOwner, _ mutate: (inout [AppRoute]) -> Void) {
        switch owner {
        case .settings:
            guard isSettingsPresented else { return }
            mutate(&settingsPath)
        case let .tab(tab):
            var path = path(for: tab)
            mutate(&path)
            setPath(path, for: tab)
        }
    }

    private static func isConnectRoute(_ route: AppRoute) -> Bool {
        switch route {
        case .connectTrueLayer, .connectEnableBanking, .connectTrading212: true
        default: false
        }
    }
}
