import SwiftUI

extension View {
    // The active fold of a partially open iPhone Duo, in this view's coordinates, when it runs top to bottom.
    func onVerticalFoldChange(_ action: @escaping (CGRect?) -> Void) -> some View {
        modifier(VerticalFold(action: action))
    }
}

// Between two columns: a hairline, or the whole fold while the device is partially open.
struct ColumnSeparator: View {
    let fold: CGRect?

    var body: some View {
        if let fold {
            Color.screenBackground
                .frame(width: fold.width)
                .ignoresSafeArea()
        } else {
            Divider()
                .ignoresSafeArea()
        }
    }
}

private struct VerticalFold: ViewModifier {
    let action: (CGRect?) -> Void

    func body(content: Content) -> some View {
        // #available alone does not compile against the iOS 26 SDK, which other worktrees still build with.
        #if compiler(>=6.4)
        if #available(iOS 27.1, *) {
            content.onGeometryChange(for: CGRect?.self) { proxy in
                proxy.reservedRegions(kind: .division)
                    .first { $0.isActive && $0.frame.height > $0.frame.width && $0.frame.minX > 0 && $0.frame.maxX < proxy.size.width }?
                    .frame
            } action: { action($0) }
        } else {
            content
        }
        #else
        content
        #endif
    }
}
