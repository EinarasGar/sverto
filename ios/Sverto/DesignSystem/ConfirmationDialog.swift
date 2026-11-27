import SwiftUI

enum DialogSource {
    case screen
    case trigger
}

// A screen applies the same dialog twice: where it always was (.screen) and on the control that opens it (.trigger).
// Only one of them presents: the screen at compact width, so an iPhone shows it exactly as before, and the trigger at
// regular width, so the popover points at that control.
struct ConfirmationDialog<Item, Actions: View, Message: View>: ViewModifier {
    let source: DialogSource
    let title: String
    @Binding var isPresented: Bool
    let item: Item?
    @ViewBuilder let actions: (Item) -> Actions
    @ViewBuilder let message: (Item) -> Message

    @Environment(\.horizontalSizeClass) private var sizeClass

    func body(content: Content) -> some View {
        content.confirmationDialog(
            title,
            isPresented: (sizeClass == .regular) == (source == .trigger) ? $isPresented : .constant(false),
            titleVisibility: .visible,
            presenting: item,
            actions: actions,
            message: message
        )
    }
}

extension ConfirmationDialog where Item == Void {
    init(
        _ source: DialogSource,
        title: String,
        isPresented: Binding<Bool>,
        @ViewBuilder actions: @escaping () -> Actions,
        @ViewBuilder message: @escaping () -> Message
    ) {
        self.init(source: source, title: title, isPresented: isPresented, item: (), actions: { actions() }, message: { message() })
    }
}

extension ConfirmationDialog where Item == Void, Message == EmptyView {
    init(_ source: DialogSource, title: String, isPresented: Binding<Bool>, @ViewBuilder actions: @escaping () -> Actions) {
        self.init(source: source, title: title, isPresented: isPresented, item: (), actions: { actions() }, message: { EmptyView() })
    }
}

extension ConfirmationDialog {
    init(
        _ source: DialogSource,
        title: String,
        presenting item: Binding<Item?>,
        @ViewBuilder actions: @escaping (Item) -> Actions,
        @ViewBuilder message: @escaping (Item) -> Message
    ) {
        self.init(
            source: source,
            title: title,
            isPresented: Binding(get: { item.wrappedValue != nil }, set: { if !$0 { item.wrappedValue = nil } }),
            item: item.wrappedValue,
            actions: actions,
            message: message
        )
    }
}
