import SwiftUI

struct AiConsentDisclosure: View {
    @ScaledMetric(relativeTo: .subheadline) private var iconWidth: CGFloat = 28

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(AiConsentCopy.intro)
                .font(.body)
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 14) {
                ForEach(AiConsentCopy.sharedData, id: \.text) { item in
                    Label {
                        Text(item.text)
                            .font(.subheadline)
                    } icon: {
                        Image(systemName: item.systemImage)
                            .foregroundStyle(.tint)
                            .frame(width: iconWidth)
                    }
                }
            }
            .padding(Layout.cardPadding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(.tertiarySystemFill), in: .rect(cornerRadius: Layout.cardRadius))
            Text(AiConsentCopy.footer)
                .font(.footnote)
                .foregroundStyle(.secondary)
            Link(AiConsentCopy.privacyPolicyTitle, destination: AiConsentCopy.privacyPolicyURL)
                .font(.footnote.weight(.semibold))
                .frame(minHeight: 44)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
    }
}

#Preview {
    AiConsentDisclosure()
        .padding(24)
}
