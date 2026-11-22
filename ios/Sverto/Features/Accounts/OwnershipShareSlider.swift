import SwiftUI

struct OwnershipShareSlider: View {
    @Binding var share: Double
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var percentLabel: String {
        "\(Int((share * 100).rounded()))%"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(percentLabel)
                .font(.title2.weight(.semibold))
                .monospacedDigit()
                .foregroundStyle(.tint)
                .contentTransition(reduceMotion ? .identity : .numericText())
                .animation(reduceMotion ? nil : .snappy, value: share)
            Slider(value: $share, in: 0.1...1.0, step: 0.1) {
                Text("Ownership share")
            } minimumValueLabel: {
                Text("10%")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            } maximumValueLabel: {
                Text("100%")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .accessibilityValue(percentLabel)
        }
        .padding(.vertical, 4)
    }
}
