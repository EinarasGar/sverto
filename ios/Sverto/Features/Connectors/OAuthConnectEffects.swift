import SwiftUI

extension View {
    func oauthConnectEffects(
        model: OAuthConnectViewModel,
        webSession: OAuthWebSession,
        oauthState: String?,
        oauthCode: String?,
        oauthError: String?
    ) -> some View {
        modifier(OAuthConnectEffects(model: model, webSession: webSession, oauthState: oauthState, oauthCode: oauthCode, oauthError: oauthError))
    }
}

private struct OAuthConnectEffects: ViewModifier {
    let model: OAuthConnectViewModel
    let webSession: OAuthWebSession
    let oauthState: String?
    let oauthCode: String?
    let oauthError: String?
    @Environment(ShellState.self) private var shell

    func body(content: Content) -> some View {
        content
            .task(id: oauthState) {
                guard let oauthState else { return }
                await model.complete(state: oauthState, code: oauthCode, error: oauthError)
            }
            .onChange(of: model.launchUrl) { _, url in
                guard let url else { return }
                model.consumeLaunchUrl()
                Task { await launch(url) }
            }
            .onChange(of: model.phase) { _, phase in
                guard phase == .completed, let connectionId = model.connectionId else { return }
                model.consumeCompleted()
                shell.finishConnecting(connectionId: connectionId, in: shell.navigationOwner)
            }
            .sensoryFeedback(.success, trigger: model.phase) { _, phase in phase == .completed }
            .sensoryFeedback(.error, trigger: model.phase) { _, phase in phase == .failed }
            .onDisappear { webSession.cancel() }
    }

    private func launch(_ urlString: String) async {
        guard let url = URL(string: urlString) else {
            model.fail("The bank returned an invalid authorisation link.")
            return
        }
        do {
            guard let callbackURL = try await webSession.authenticate(url: url),
                  let callback = OAuthCallback(url: callbackURL) else { return }
            await model.complete(state: callback.state, code: callback.code, error: callback.error)
        } catch {
            model.fail("Couldn't open your bank's sign-in page. Try again.")
        }
    }
}
