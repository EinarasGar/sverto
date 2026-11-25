import SvertoCore
import SwiftUI

struct ConnectTrading212Screen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        ConnectTrading212Content(store: services.store)
    }
}

private struct ConnectTrading212Content: View {
    @State private var model: ConnectTrading212ViewModel
    @State private var mode: Trading212Mode = .stored
    @State private var apiKeyId = ""
    @State private var apiKey = ""
    @FocusState private var isKeyFocused: Bool
    @Environment(ShellState.self) private var shell

    init(store: AppStore) {
        _model = State(initialValue: ConnectTrading212ViewModel(store: store))
    }

    private var canConnect: Bool {
        mode.credentialMode != nil
            && !apiKey.trimmingCharacters(in: .whitespaces).isEmpty
            && !model.submitting
    }

    var body: some View {
        Form {
            Section {
                Picker("Where the key is kept", selection: $mode) {
                    ForEach(Trading212Mode.allCases, id: \.self) { entry in
                        Text(entry.label).tag(entry)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                Text(mode.explanation)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            if mode.credentialMode != nil {
                Section("API key") {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("API key ID")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                        TextField("API key ID", text: $apiKeyId)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .submitLabel(.next)
                            .onSubmit { isKeyFocused = true }
                    }
                    VStack(alignment: .leading, spacing: 4) {
                        Text("API key")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                        SecureField("API key", text: $apiKey)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .focused($isKeyFocused)
                            .submitLabel(.done)
                            .onSubmit { isKeyFocused = false }
                    }
                }
            }
            if let error = model.error {
                Section {
                    FormErrorRow(message: error)
                }
            }
        }
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle("Connect Trading 212")
        .navigationBarTitleDisplayMode(.inline)
        .safeAreaInset(edge: .bottom) {
            ConnectorHeroButton(title: mode == .client ? "Coming soon" : "Connect", isBusy: model.submitting) {
                Task { await connect() }
            }
            .disabled(!canConnect)
        }
        .sensoryFeedback(.selection, trigger: mode)
        .sensoryFeedback(.error, trigger: model.error) { _, error in error != nil }
    }

    private func connect() async {
        guard let credentialMode = mode.credentialMode else { return }
        let owner = shell.navigationOwner
        if let connectionId = await model.connect(mode: credentialMode, apiKeyId: apiKeyId, apiKey: apiKey) {
            shell.finishConnecting(connectionId: connectionId, in: owner)
        }
    }
}
