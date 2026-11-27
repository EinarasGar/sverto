import SwiftUI
import UIKit

// A tab root is rebuilt when it moves between the stack and the list column (a fold, a window resize), so the visible
// copy records its scroll offset here and a new copy starts from it. A class, so recording does not redraw the tab.
final class RootScrollMemory {
    var offset: CGFloat = 0
}

extension EnvironmentValues {
    @Entry var rootScroll: RootScrollMemory?
}

extension View {
    func keepsRootScrollOffset() -> some View {
        modifier(KeepsRootScrollOffset())
    }
}

private struct KeepsRootScrollOffset: ViewModifier {
    @Environment(\.rootScroll) private var memory
    @State private var anchor = ScrollViewAnchor.Reference()
    @State private var isRestored = false

    func body(content: Content) -> some View {
        content
            .background(ScrollViewAnchor(reference: anchor))
            .onScrollGeometryChange(for: ScrollGeometry.self) { $0 } action: { _, geometry in
                guard let memory else { return }
                if isRestored {
                    memory.offset = geometry.contentOffset.y + geometry.contentInsets.top
                } else if geometry.contentSize.height > geometry.containerSize.height {
                    isRestored = true
                    let offset = min(memory.offset, geometry.contentSize.height - geometry.containerSize.height + geometry.contentInsets.bottom)
                    // List ignores ScrollPosition, so the offset is set on the collection view behind it.
                    if offset > 1, let scrollView = anchor.scrollView() {
                        Task { scrollView.contentOffset.y = offset - scrollView.adjustedContentInset.top }
                    }
                }
            }
    }
}

private struct ScrollViewAnchor: UIViewRepresentable {
    final class Reference {
        weak var view: UIView?

        func scrollView() -> UIScrollView? {
            var ancestor = view?.superview
            while let current = ancestor {
                if let found = current.firstScrollView() { return found }
                ancestor = current.superview
            }
            return nil
        }
    }

    let reference: Reference

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.isUserInteractionEnabled = false
        reference.view = view
        return view
    }

    func updateUIView(_ uiView: UIView, context: Context) {}
}

private extension UIView {
    func firstScrollView() -> UIScrollView? {
        for subview in subviews {
            if let scrollView = subview as? UIScrollView { return scrollView }
        }
        for subview in subviews {
            if let found = subview.firstScrollView() { return found }
        }
        return nil
    }
}
