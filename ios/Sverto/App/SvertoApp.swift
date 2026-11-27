import ClerkKit
import SwiftUI
import SvertoCore

@main
struct SvertoApp: App {
    @State private var services: AppServices
    @State private var session: SessionCoordinator
    @Environment(\.scenePhase) private var scenePhase

    init() {
        let environment = AppEnvironment.current
        if environment.isClerkConfigured {
            Clerk.configure(publishableKey: environment.clerkPublishableKey)
        }
        let clerk = ClerkGateway(isConfigured: environment.isClerkConfigured)
        let connectivityProbe = ConnectivityProbe()
        let authProvider = ClerkAuthProvider(isConfigured: environment.isClerkConfigured, hasConnectivity: connectivityProbe.isConnected)
        let store = AppStoreFactory.make(environment: environment, authProvider: authProvider, credentialStore: KeychainCredentialStore())
        let connectivity = ConnectivityMonitor(store: store, probe: connectivityProbe)
        let services = AppServices(environment: environment, store: store, connectivity: connectivity, clerk: clerk)
        let session = SessionCoordinator(
            store: store,
            environment: environment,
            clerk: clerk,
            authProvider: authProvider,
            shell: services.shell,
            hasConnectivity: connectivityProbe.isConnected
        )
        _services = State(initialValue: services)
        _session = State(initialValue: session)
        connectivity.start()
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(services)
                .environment(session)
                .environment(services.shell)
                .environment(services.connection)
                .environment(services.clerk)
                .modifier(ClerkEnvironment(isConfigured: services.environment.isClerkConfigured))
                .tint(Color.accentColor)
        }
        .commands { AppCommands() }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active {
                services.connectivity.refreshAfterForeground()
            }
        }
    }
}

private struct ClerkEnvironment: ViewModifier {
    let isConfigured: Bool

    func body(content: Content) -> some View {
        if isConfigured {
            content.environment(Clerk.shared)
        } else {
            content
        }
    }
}
