import SwiftUI

enum ImportTone {
    case ghost
    case brand
    case attention
    case positive
    case negative

    var tint: Color {
        switch self {
        case .ghost: .secondary
        case .brand: .brand
        case .attention: Color(.systemOrange)
        case .positive: .gain
        case .negative: .loss
        }
    }

    var fill: Color {
        switch self {
        case .ghost: Color(.tertiarySystemFill)
        default: tint.opacity(Layout.iconTileOpacity)
        }
    }
}

struct ToneChip: View {
    let label: String
    let tone: ImportTone
    var systemImage: String? = nil

    var body: some View {
        HStack(spacing: 6) {
            if let systemImage {
                Image(systemName: systemImage)
                    .font(.caption2.weight(.bold))
            } else {
                Circle()
                    .fill(tone.tint)
                    .frame(width: 6, height: 6)
            }
            Text(label)
                .font(.caption.weight(.medium))
                .lineLimit(1)
        }
        .foregroundStyle(tone.tint)
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(tone.fill, in: .capsule)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
    }
}

#Preview {
    VStack(alignment: .leading, spacing: 8) {
        ToneChip(label: "Ready to review", tone: .positive)
        ToneChip(label: "Balanced", tone: .positive, systemImage: "checkmark")
        ToneChip(label: "Off by £12.00", tone: .negative)
        ToneChip(label: "Waiting for files", tone: .ghost)
    }
    .padding()
}
