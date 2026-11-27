import SwiftUI

struct ListDetailStack<Root: View>: View {
    @Binding var path: [AppRoute]
    let placeholderTitle: String
    let placeholderSystemImage: String
    @ViewBuilder let root: (_ isListColumn: Bool) -> Root

    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(ToastCenter.self) private var toasts
    @State private var width = CGFloat.infinity
    @State private var fold: CGRect?
    @State private var rootScroll = RootScrollMemory()

    private var showsColumns: Bool {
        sizeClass == .regular && width >= Layout.minListDetailWidth
    }

    var body: some View {
        let showsList = showsColumns && path.last?.takesFocus != true
        // The path-bound stack must stay at the same position in this HStack in every layout: a fold or a window
        // resize only adds or removes the list column beside it, so pushed screens keep their state.
        HStack(spacing: 0) {
            if showsColumns {
                NavigationStack {
                    root(true)
                        // On iPhone Duo the system gives this stack the vertical bar's inset as well, although only
                        // the trailing stack's bars move into the strip.
                        .ignoresSafeArea(.container, edges: .trailing)
                        .environment(\.rootScroll, rootScroll)
                }
                .frame(width: showsList ? fold?.minX ?? Layout.listColumnWidth(in: width) : 0)
                .opacity(showsList ? 1 : 0)
                .allowsHitTesting(showsList)
                .accessibilityHidden(!showsList)
                if showsList {
                    ColumnSeparator(fold: fold)
                }
            }
            NavigationStack(path: $path) {
                Group {
                    if showsColumns {
                        ContentUnavailableView(placeholderTitle, systemImage: placeholderSystemImage)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                            .background(.screenBackground)
                    } else {
                        root(false)
                            .environment(\.rootScroll, rootScroll)
                    }
                }
                .offlineBannerInset()
                .navigationDestination(for: AppRoute.self) { RouteDestination(route: $0) }
            }
            .environment(\.detailColumnRoot, showsColumns ? path.first : nil)
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width.rounded() } action: { width = $0 }
        .onVerticalFoldChange { fold = $0 }
        .toastHost(toasts)
    }
}

extension EnvironmentValues {
    @Entry var detailColumnRoot: AppRoute?
}
