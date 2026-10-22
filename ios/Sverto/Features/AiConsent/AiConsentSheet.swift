import SwiftUI

extension View {
    func aiConsentSheet(_ gate: AiConsentGate) -> some View {
        modifier(AiConsentSheetModifier(gate: gate))
    }
}

private struct AiConsentSheetModifier: ViewModifier {
    @Bindable var gate: AiConsentGate

    func body(content: Content) -> some View {
        content.sheet(isPresented: $gate.isPresented, onDismiss: gate.sheetDismissed) {
            AiConsentSheet(gate: gate)
        }
    }
}

struct AiConsentSheet: View {
    let gate: AiConsentGate

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                Image(systemName: "sparkles")
                    .font(.title.weight(.medium))
                    .imageScale(.large)
                    .foregroundStyle(.tint)
                    .frame(width: 72, height: 72)
                    .background(Color.accentColor.opacity(0.15), in: .circle)
                    .accessibilityHidden(true)
                Text(AiConsentCopy.popupTitle)
                    .font(.title2.bold())
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                AiConsentDisclosure()
            }
            .padding(.horizontal, 24)
            .padding(.top, 32)
            .padding(.bottom, 8)
        }
        .scrollBounceBehavior(.basedOnSize)
        .safeAreaBar(edge: .bottom) {
            VStack(spacing: 12) {
                if let error = gate.error {
                    FormErrorRow(message: error)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                ProminentActionButton(title: AiConsentCopy.turnOn, isBusy: gate.isEnabling) {
                    Task { await gate.turnOn() }
                }
                Button(AiConsentCopy.popupDecline, action: gate.decline)
                    .frame(minHeight: 44)
                    .disabled(gate.isEnabling)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 12)
        }
        .interactiveDismissDisabled(gate.isEnabling)
        .onChange(of: gate.error) { _, error in
            if let error { AccessibilityNotification.Announcement(error).post() }
        }
    }
}
