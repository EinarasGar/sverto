import SwiftUI

struct AiFeaturesSection: View {
    let model: AiFeaturesViewModel
    @Environment(ToastCenter.self) private var toasts

    private var isOn: Binding<Bool> {
        Binding(
            get: { model.isEnabled },
            set: { enabled in
                if enabled {
                    model.turnOn()
                } else {
                    Task { await turnOff() }
                }
            }
        )
    }

    var body: some View {
        Section {
            Toggle(isOn: isOn) {
                HStack(spacing: 12) {
                    SettingsIcon(systemImage: "sparkles")
                    VStack(alignment: .leading, spacing: 2) {
                        Text(AiConsentCopy.settingsTitle)
                        Text(AiConsentCopy.settingsDescription)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            .disabled(model.isSaving)
        }
    }

    private func turnOff() async {
        do {
            try await model.turnOff()
        } catch {
            toasts.showError(error.friendlyMessage)
        }
    }
}
