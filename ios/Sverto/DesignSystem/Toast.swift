import Observation
import SwiftUI

@Observable
final class ToastCenter {
    private(set) var message: String?
    private var dismissTask: Task<Void, Never>?

    func show(_ message: String) {
        dismissTask?.cancel()
        self.message = message
        AccessibilityNotification.Announcement(message).post()
        dismissTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled else { return }
            self?.message = nil
        }
    }

    func showError(_ message: String) {
        Haptics.error()
        show(message)
    }

    func dismiss() {
        dismissTask?.cancel()
        message = nil
    }
}

extension View {
    func toastHost(_ center: ToastCenter) -> some View {
        modifier(ToastHost(center: center))
    }

    func toastOnError<Event: Equatable>(
        _ event: Event?,
        message: @escaping (Event) -> String,
        onShown: @escaping () -> Void = {}
    ) -> some View {
        modifier(ErrorToast(event: event, message: message, onShown: onShown))
    }

    func toastOnError(_ message: String?, onShown: @escaping () -> Void = {}) -> some View {
        toastOnError(message, message: { $0 }, onShown: onShown)
    }
}

private struct ToastHost: ViewModifier {
    let center: ToastCenter
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            .overlay(alignment: .bottom) {
                if let message = center.message {
                    Text(message)
                        .font(.subheadline.weight(.medium))
                        .multilineTextAlignment(.center)
                        .lineLimit(3)
                        .padding(.horizontal, 20)
                        .padding(.vertical, 12)
                        .glassEffect()
                        .padding(.horizontal, 24)
                        .padding(.bottom, 12)
                        .transition(reduceMotion ? .opacity : .move(edge: .bottom).combined(with: .opacity))
                        .onTapGesture { center.dismiss() }
                }
            }
            .animation(reduceMotion ? nil : .snappy, value: center.message)
    }
}

private struct ErrorToast<Event: Equatable>: ViewModifier {
    let event: Event?
    let message: (Event) -> String
    let onShown: () -> Void
    @Environment(ToastCenter.self) private var toasts

    func body(content: Content) -> some View {
        content.onChange(of: event) { _, event in
            guard let event else { return }
            toasts.showError(message(event))
            onShown()
        }
    }
}
