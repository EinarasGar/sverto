import Foundation
import Observation
import SvertoCore

@Observable
final class SessionCoordinator {
    private(set) var state: SessionState = .loading
    private(set) var serverInfo: ServerInfo?
    private(set) var serverUrl: String
    private(set) var serverOrigin: ServerOrigin?
    private var signedInGeneration = 0

    private let store: AppStore
    private let environment: AppEnvironment
    private let preferences: ServerPreferences
    private let clerk: ClerkGateway
    private let authProvider: ClerkAuthProvider
    private let shell: ShellState
    private let hasConnectivity: @Sendable () -> Bool
    private var authBridge: AuthObserverBridge?

    var authMode: AuthMode? { serverInfo?.authMode }
    var isSelfHosted: Bool { serverOrigin == .selfHosted }

    init(
        store: AppStore,
        environment: AppEnvironment,
        clerk: ClerkGateway,
        authProvider: ClerkAuthProvider,
        shell: ShellState,
        hasConnectivity: @escaping @Sendable () -> Bool,
        preferences: ServerPreferences = ServerPreferences()
    ) {
        self.store = store
        self.environment = environment
        self.clerk = clerk
        self.authProvider = authProvider
        self.shell = shell
        self.hasConnectivity = hasConnectivity
        self.preferences = preferences
        serverUrl = environment.apiBaseURL.absoluteString
    }

    func start() async {
        observeAuthExpiry()
        if preferences.welcomeRequired == true {
            state = .welcome
            return
        }
        if let active = preferences.activeServer {
            serverUrl = active.url
            serverOrigin = active.origin ?? inferOrigin(active.url)
            serverInfo = ServerInfo(authMode: active.authMode, version: active.version)
            await resumeServer(url: active.url, mode: active.authMode)
            return
        }
        let store = self.store
        if await Task.detached(operation: { store.getCachedMe() }).value != nil {
            let mode: AuthMode = environment.isClerkConfigured ? .clerk : .noauth
            let url = environment.apiBaseURL.absoluteString
            preferences.setActiveServer(url: url, authMode: mode, origin: .hosted, version: "")
            serverUrl = url
            serverOrigin = .hosted
            serverInfo = ServerInfo(authMode: mode, version: "")
            await resumeServer(url: url, mode: mode)
            return
        }
        state = .welcome
    }

    private func observeAuthExpiry() {
        guard authBridge == nil else { return }
        let bridge = AuthObserverBridge { [weak self] in
            Task { @MainActor [weak self] in await self?.handleSessionExpired() }
        }
        authBridge = bridge
        let store = self.store
        Task.detached { store.observeAuth(observer: bridge) }
    }

    private func resumeServer(url: String, mode: AuthMode) async {
        await store.resumeServer(url: url, authMode: mode)
        await gate(by: mode)
    }

    private func gate(by mode: AuthMode) async {
        switch mode {
        case .noauth:
            enterSignedIn()
        case .database:
            state = store.hasSession() ? signedInState() : .signIn(isSubmitting: false, errorMessage: nil)
        case .clerk:
            let store = self.store
            let cachedMe = await Task.detached(operation: { store.restoreCachedMe() }).value
            if !hasConnectivity(), cachedMe != nil {
                enterSignedIn()
                return
            }
            await clerk.waitUntilLoaded()
            state = clerk.userId != nil ? signedInState() : .clerkLogin
        }
    }

    private func signedInState() -> SessionState {
        signedInGeneration += 1
        return .signedIn(generation: signedInGeneration)
    }

    private func enterSignedIn() {
        state = signedInState()
    }

    func showWelcome() {
        state = .welcome
    }

    func continueWithHosted() {
        let url = environment.apiBaseURL.absoluteString
        state = .connecting
        Task {
            do {
                let info = try await store.connectServer(url: url)
                serverInfo = info
                serverUrl = url
                serverOrigin = .hosted
                preferences.setActiveServer(url: url, authMode: info.authMode, origin: .hosted, version: info.version)
                await gate(by: info.authMode)
            } catch {
                state = .connectError(message: String(format: ServerCopy.hostedUnreachable, url))
            }
        }
    }

    func retryHostedConnect() {
        continueWithHosted()
    }

    func showServerUrlInput() {
        state = .serverUrl(lastUrl: preferences.lastSelfHostedUrl ?? "", status: .idle)
    }

    func checkSelfHostedServer(_ rawUrl: String) {
        let url = normalizedServerUrl(rawUrl)
        guard case let .serverUrl(lastUrl, _) = state else { return }
        state = .serverUrl(lastUrl: lastUrl, status: .checking(url: url))
        Task {
            do {
                let info = try await store.probeServer(url: url)
                guard case let .serverUrl(current, .checking(checkingUrl)) = state, checkingUrl == url else { return }
                state = .serverUrl(lastUrl: current, status: .found(url: url, info: info))
            } catch {
                guard case let .serverUrl(current, .checking(checkingUrl)) = state, checkingUrl == url else { return }
                state = .serverUrl(lastUrl: current, status: .error(url: url, message: ServerCopy.serverNotFound))
            }
        }
    }

    func continueWithSelfHosted() {
        guard case let .serverUrl(_, .found(url, info)) = state else { return }
        state = .connecting
        Task {
            serverInfo = info
            serverUrl = url
            serverOrigin = .selfHosted
            await store.resumeServer(url: url, authMode: info.authMode)
            preferences.setActiveServer(url: url, authMode: info.authMode, origin: .selfHosted, version: info.version)
            preferences.setLastSelfHostedUrl(url)
            await gate(by: info.authMode)
        }
    }

    func showSignIn() {
        state = .signIn(isSubmitting: false, errorMessage: nil)
    }

    func showSignUp() {
        state = .signUp
    }

    func signIn(username: String, password: String) {
        state = .signIn(isSubmitting: true, errorMessage: nil)
        Task {
            do {
                try await store.signInWithPassword(username: username, password: password)
                enterSignedIn()
            } catch {
                state = .signIn(isSubmitting: false, errorMessage: signInErrorMessage(error))
            }
        }
    }

    func signUp(username: String, password: String) {
        state = .connecting
        Task {
            do {
                try await store.signUpWithPassword(username: username, password: password)
                enterSignedIn()
            } catch {
                state = .signIn(isSubmitting: false, errorMessage: signInErrorMessage(error))
            }
        }
    }

    private func signInErrorMessage(_ error: Error) -> String {
        guard let apiError = error as? ApiError else { return ServerCopy.signInUnknown }
        switch apiError {
        case let .Server(reason, status):
            if (401...403).contains(Int(status)) { return ServerCopy.badCredentials }
            return reason.isEmpty ? ServerCopy.signInServerFailure : reason
        case .Network: return ServerCopy.signInNetwork
        case .Timeout: return ServerCopy.signInTimeout
        case .Parse: return ServerCopy.signInParse
        case .MissingLocalCredential: return ServerCopy.signInMissingCredential
        case .AiFeaturesDisabled: return ServerCopy.signInServerFailure
        }
    }

    func onClerkSignedIn() {
        enterSignedIn()
    }

    func handleClerkSignedOut() {
        guard authMode == .clerk, case .signedIn = state else { return }
        returnToWelcome()
    }

    func signOut() {
        let wasClerk = authMode == .clerk
        returnToWelcome()
        if wasClerk {
            Task { await clerk.signOut() }
        }
    }

    func useDifferentServer() {
        endSession()
        preferences.clearActiveServer()
        serverInfo = nil
        serverOrigin = nil
        state = .welcome
    }

    func deleteUserAndSignOut() async -> Error? {
        do {
            try await store.deleteUser()
        } catch {
            return error
        }
        let wasClerk = authMode == .clerk
        returnToWelcome()
        if wasClerk {
            await clerk.signOut()
        }
        return nil
    }

    private func returnToWelcome() {
        endSession()
        let store = self.store
        Task.detached { await store.signOut() }
        preferences.clearActiveServer()
        serverInfo = nil
        serverOrigin = nil
        state = .welcome
    }

    private func handleSessionExpired() async {
        endSession()
        await store.signOut()
        state = authMode == .clerk ? .clerkLogin : .signIn(isSubmitting: false, errorMessage: nil)
    }

    private func endSession() {
        authProvider.invalidate()
        shell.reset()
    }

    private func inferOrigin(_ url: String) -> ServerOrigin {
        url == environment.apiBaseURL.absoluteString ? .hosted : .selfHosted
    }

    private func normalizedServerUrl(_ raw: String) -> String {
        var url = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        while url.hasSuffix("/") { url.removeLast() }
        return url
    }

    static func isPlausibleServerUrl(_ raw: String) -> Bool {
        guard let components = URLComponents(string: raw.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = components.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = components.host, !host.isEmpty else { return false }
        return true
    }
}
