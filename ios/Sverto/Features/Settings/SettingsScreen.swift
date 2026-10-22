import ClerkKitUI
import SvertoCore
import SwiftUI

struct SettingsScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        SettingsList(store: services.store)
    }
}

private struct SettingsList: View {
    @State private var model: SettingsViewModel
    @State private var aiUsage: AiUsageViewModel
    @State private var aiFeatures: AiFeaturesViewModel
    @State private var isCurrencyPickerPresented = false
    @State private var isServerSheetPresented = false
    @State private var isExportsPresented = false
    @State private var isProfilePresented = false
    @Environment(SessionCoordinator.self) private var session
    @Environment(ClerkGateway.self) private var clerk
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    private let store: AppStore

    init(store: AppStore) {
        self.store = store
        _model = State(initialValue: SettingsViewModel(store: store))
        _aiUsage = State(initialValue: AiUsageViewModel(store: store))
        _aiFeatures = State(initialValue: AiFeaturesViewModel(store: store))
    }

    private var authMode: AuthMode { session.authMode ?? .noauth }
    private var isClerk: Bool { authMode == .clerk }

    var body: some View {
        List {
            SettingsAccountHeader(
                displayName: model.displayName(authMode: authMode, clerkName: clerk.displayName),
                email: isClerk ? clerk.email : nil,
                initials: isClerk ? clerk.initials : nil,
                imageURL: isClerk ? clerk.imageURL : nil
            )
            Section {
                SettingsRow(title: "Custom Categories", systemImage: "tag", showsChevron: true) {
                    shell.push(.customCategories)
                }
                SettingsRow(title: "Custom Assets", systemImage: "chart.pie", showsChevron: true) {
                    shell.push(.customAssets)
                }
                SettingsRow(title: "Base Currency", systemImage: "dollarsign.circle", value: model.baseCurrencyTicker) {
                    isCurrencyPickerPresented = true
                }
                SettingsRow(title: "Connected Services", systemImage: "cable.connector", showsChevron: true) {
                    shell.push(.connectors)
                }
            }
            Section {
                if session.isSelfHosted {
                    SettingsRow(title: "Server", systemImage: "server.rack", value: session.serverUrl) {
                        isServerSheetPresented = true
                    }
                }
                SettingsRow(title: "Export ledger", systemImage: "square.and.arrow.down") {
                    isExportsPresented = true
                }
                if isClerk, clerk.isConfigured {
                    SettingsRow(
                        title: "Profile & account",
                        systemImage: "person.crop.circle",
                        subtitle: "Manage your profile and sign out",
                        showsChevron: true
                    ) {
                        isProfilePresented = true
                    }
                } else if authMode == .database {
                    SettingsRow(title: "Sign out", systemImage: "rectangle.portrait.and.arrow.right") {
                        session.signOut()
                        shell.dismissSettings()
                    }
                }
            }
            AiFeaturesSection(model: aiFeatures)
            if aiFeatures.isEnabled {
                AiUsageSection(model: aiUsage)
            }
            if authMode != .noauth {
                DeleteAccountSection()
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Settings")
        .navigationBarTitleDisplayMode(.large)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Done") { shell.dismissSettings() }
            }
        }
        .task { await model.load() }
        .task(id: aiFeatures.isEnabled) {
            if aiFeatures.isEnabled { await aiUsage.load() }
        }
        .onAppear { aiFeatures.refresh() }
        .aiConsentSheet(aiFeatures.aiConsent)
        .sheet(isPresented: $isCurrencyPickerPresented) {
            CurrencyPickerSheet(title: "Choose base currency", selectedId: model.baseCurrencyId) { asset in
                Task { await selectBaseCurrency(asset) }
            }
        }
        .sheet(isPresented: $isServerSheetPresented) {
            ServerSettingsSheet(serverUrl: session.serverUrl, version: session.serverInfo?.version ?? "", authMode: authMode) {
                isServerSheetPresented = false
                session.useDifferentServer()
                shell.dismissSettings()
            }
        }
        .sheet(isPresented: $isExportsPresented) {
            ExportsSheet(store: store)
        }
        .sheet(isPresented: $isProfilePresented) {
            UserProfileView(isDismissible: true)
        }
    }

    private func selectBaseCurrency(_ asset: AssetItem) async {
        do {
            try await model.selectBaseCurrency(asset)
        } catch {
            toasts.show(error.friendlyMessage)
        }
    }
}
