import SwiftUI

// iPhone Duo's vertical toolbar wants a title with each symbol; elsewhere this stays a plain Image, because a Label
// draws the symbol lighter and smaller in an iPhone's horizontal toolbar.
struct ToolbarSymbol: View {
    let title: String
    let systemImage: String

    init(_ title: String, systemImage: String) {
        self.title = title
        self.systemImage = systemImage
    }

    var body: some View {
        #if compiler(>=6.4)
        if #available(iOS 27.1, *) {
            VerticalAwareSymbol(title: title, systemImage: systemImage)
        } else {
            Image(systemName: systemImage)
        }
        #else
        Image(systemName: systemImage)
        #endif
    }
}

#if compiler(>=6.4)
@available(iOS 27.1, *)
private struct VerticalAwareSymbol: View {
    let title: String
    let systemImage: String
    @Environment(\.toolbarVerticalEdge) private var verticalEdge

    var body: some View {
        if verticalEdge == nil {
            Image(systemName: systemImage)
        } else {
            Label(title, systemImage: systemImage)
        }
    }
}
#endif
