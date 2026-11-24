import SwiftUI
import SvertoCore
import Testing
import UIKit
@testable import Sverto

@MainActor
struct TabRootTests {
    @Test func quickUploadQueueSurvivesHostReconstruction() async throws {
        let databaseURL = FileManager.default.temporaryDirectory.appendingPathComponent("QuickUploads.\(UUID().uuidString).sqlite")
        defer { try? FileManager.default.removeItem(at: databaseURL) }
        let store = AppStore(
            baseUrl: "http://127.0.0.1:1",
            cacheTtlSecs: 60,
            dbPath: databaseURL.path,
            authProvider: ClerkAuthProvider(isConfigured: false, hasConnectivity: { false }),
            credentialStore: KeychainCredentialStore(service: "QuickUploads.\(UUID().uuidString)")
        )
        store.setConnectivity(connected: false)
        var observedModels: [QuickUploadViewModel] = []
        func root(revision: Int) -> some View {
            TransactionsModelsHost(store: store) {
                QuickUploadModelProbe(revision: revision) { observedModels.append($0) }
            }
        }
        let host = UIHostingController(rootView: root(revision: 0))
        let scene = try #require(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        host.view.layoutIfNeeded()
        let initialDeadline = ContinuousClock.now + .seconds(2)
        while observedModels.isEmpty, ContinuousClock.now < initialDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        let retainedModel = try #require(observedModels.first)

        store.queueQuickUpload(data: Data([1]), thumbnail: Data(), mimeType: "image/png")
        let firstQueueDeadline = ContinuousClock.now + .seconds(2)
        while retainedModel.items.count != 1, ContinuousClock.now < firstQueueDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(retainedModel.items.count == 1)

        host.rootView = root(revision: 1)
        host.view.layoutIfNeeded()
        let renderDeadline = ContinuousClock.now + .seconds(2)
        while observedModels.count < 2, ContinuousClock.now < renderDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(observedModels.count == 2)
        #expect(observedModels.last === retainedModel)

        store.queueQuickUpload(data: Data([2]), thumbnail: Data(), mimeType: "image/png")
        let updateDeadline = ContinuousClock.now + .seconds(2)
        while retainedModel.items.count != 2, ContinuousClock.now < updateDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(retainedModel.items.count == 2)
    }

    @Test func transactionsTabRendersWithSharedModels() throws {
        let environment = AppEnvironment(
            flavor: .dev,
            apiBaseURL: try #require(URL(string: "http://127.0.0.1:1")),
            clerkPublishableKey: ""
        )
        let store = AppStore(
            baseUrl: environment.apiBaseURL.absoluteString,
            cacheTtlSecs: 60,
            dbPath: ":memory:",
            authProvider: ClerkAuthProvider(isConfigured: false, hasConnectivity: { false }),
            credentialStore: KeychainCredentialStore(service: "TabRootTests.\(UUID().uuidString)")
        )
        store.setConnectivity(connected: false)
        let services = AppServices(
            environment: environment,
            store: store,
            connectivity: ConnectivityMonitor(store: store, probe: ConnectivityProbe()),
            clerk: ClerkGateway(isConfigured: false)
        )
        services.shell.selectedTab = .transactions
        let root = TabRoot()
            .environment(services)
            .environment(services.shell)
            .environment(services.connection)
            .environment(services.clerk)
        let host = UIHostingController(rootView: root)
        let scene = try #require(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        host.view.layoutIfNeeded()

        #expect(!host.view.subviews.isEmpty)
    }
}

private struct QuickUploadModelProbe: View {
    @Environment(QuickUploadViewModel.self) private var model
    let revision: Int
    let onModel: (QuickUploadViewModel) -> Void

    var body: some View {
        Color.clear
            .onChange(of: revision, initial: true) { _, _ in onModel(model) }
    }
}
