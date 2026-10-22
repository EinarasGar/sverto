import SwiftUI

struct AiFeaturesChoiceBar: View {
    let onChoose: (_ enabled: Bool) -> Void

    var body: some View {
        HStack(spacing: 12) {
            Button {
                onChoose(false)
            } label: {
                Text(AiConsentCopy.onboardingDecline)
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity, minHeight: 28)
            }
            .buttonStyle(.glass)
            .controlSize(.large)
            ProminentActionButton(title: AiConsentCopy.turnOn) { onChoose(true) }
        }
        .padding(.horizontal, 24)
        .padding(.top, 12)
        .padding(.bottom, 12)
    }
}
