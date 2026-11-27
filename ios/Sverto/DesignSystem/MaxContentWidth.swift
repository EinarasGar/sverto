import SwiftUI

extension View {
    func maxContentWidth(_ maxWidth: CGFloat) -> some View {
        modifier(MaxContentWidth(maxWidth: maxWidth))
    }
}

private struct MaxContentWidth: ViewModifier {
    let maxWidth: CGFloat
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var width: CGFloat = 0

    func body(content: Content) -> some View {
        // Padding the safe area insets the content while scroll views, backgrounds and bars keep the full width.
        content
            .safeAreaPadding(.horizontal, sizeClass == .regular ? max(0, (width - maxWidth) / 2) : 0)
            .onGeometryChange(for: CGFloat.self) { $0.size.width.rounded() } action: { width = $0 }
    }
}
