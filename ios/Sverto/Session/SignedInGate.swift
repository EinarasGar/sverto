import SwiftUI
import SvertoCore

struct SignedInGate: View {
    @Environment(AppServices.self) private var services
    @State private var resolved = false
    @State private var onboarded = false

    var body: some View {
        Group {
            if !resolved {
                LoadingScreen()
            } else if onboarded {
                TabRoot()
            } else {
                OnboardingFlow { onboarded = true }
                    .maxContentWidth(Layout.readableWidth)
            }
        }
        .task { await resolve() }
    }

    private func resolve() async {
        let store = services.store
        if let cached = await Task.detached(operation: { store.restoreCachedMe() }).value {
            onboarded = cached.onboardingVersion >= OnboardingSteps.currentVersion
            resolved = true
        }
        await store.onSignIn()
        let refreshed = await Task.detached(operation: { store.getCachedMe() }).value
        let version = refreshed?.onboardingVersion ?? store.getOnboardingVersion()
        onboarded = version >= OnboardingSteps.currentVersion
        resolved = true
    }
}
