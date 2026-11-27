import SwiftUI
import SvertoCore

struct RootView: View {
    @Environment(SessionCoordinator.self) private var session
    @Environment(ClerkGateway.self) private var clerk
    @Environment(ShellState.self) private var shell

    var body: some View {
        Group {
            switch session.state {
            case .loading, .connecting:
                LoadingScreen()
            case .welcome:
                WelcomeScreen()
                    .maxContentWidth(Layout.readableWidth)
            case let .serverUrl(lastUrl, status):
                ServerUrlScreen(lastUrl: lastUrl, status: status)
                    .maxContentWidth(Layout.readableWidth)
            case let .connectError(message):
                ConnectErrorScreen(message: message)
            case let .signIn(isSubmitting, errorMessage):
                PasswordSignInScreen(isSubmitting: isSubmitting, errorMessage: errorMessage)
                    .maxContentWidth(Layout.readableWidth)
            case .signUp:
                PasswordSignUpScreen()
                    .maxContentWidth(Layout.readableWidth)
            case .clerkLogin:
                WelcomeScreen()
                    .maxContentWidth(Layout.readableWidth)
            case let .signedIn(generation):
                SignedInGate()
                    .id(generation)
            }
        }
        .animation(.default, value: session.state)
        .fullScreenCover(isPresented: Binding(
            get: { session.state == .clerkLogin },
            set: { isPresented in
                if !isPresented && session.state == .clerkLogin {
                    session.showWelcome()
                }
            }
        )) {
            ClerkLoginScreen()
        }
        .task { await session.start() }
        .onChange(of: clerk.userId) { _, newValue in
            if newValue == nil {
                session.handleClerkSignedOut()
            }
        }
        .onOpenURL { url in
            guard case .signedIn = session.state else {
                stashDeepLink(url)
                return
            }
        }
    }

    // Links that arrive before the tab shell is mounted are parked on ShellState; TabRoot consumes them on appear.
    private func stashDeepLink(_ url: URL) {
        switch DeepLink.route(for: url) {
        case let .presentSettings(path):
            shell.pendingOAuthReturn = path.last
        case let .selectTab(tab):
            shell.selectedTab = tab
        case nil:
            break
        }
    }
}
