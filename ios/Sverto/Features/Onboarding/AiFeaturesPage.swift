import SwiftUI

struct AiFeaturesPage: View {
    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                content
                    .frame(maxWidth: Layout.readableWidth)
                    .padding(.horizontal, 32)
                    .padding(.vertical, 24)
                    .frame(maxWidth: .infinity, minHeight: geometry.size.height)
            }
        }
    }

    private var content: some View {
        VStack(spacing: 32) {
            Image(systemName: "sparkles")
                .font(.largeTitle.weight(.medium))
                .imageScale(.large)
                .foregroundStyle(.tint)
                .frame(width: 96, height: 96)
                .background(Color.accentColor.opacity(0.15), in: .circle)
                .accessibilityHidden(true)
            VStack(spacing: 16) {
                Text(AiConsentCopy.onboardingTitle)
                    .font(.largeTitle.bold())
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                AiConsentDisclosure()
            }
        }
    }
}
