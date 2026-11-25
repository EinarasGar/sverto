import SwiftUI

struct PasswordSignInScreen: View {
    let isSubmitting: Bool
    let errorMessage: String?

    @Environment(SessionCoordinator.self) private var session
    @State private var username = ""
    @State private var password = ""
    @State private var isPasswordVisible = false
    @State private var isErrorDismissed = false
    @FocusState private var focusedField: Field?

    private enum Field {
        case username
        case password
    }

    private var trimmedUsername: String { username.trimmingCharacters(in: .whitespacesAndNewlines) }

    private var canSubmit: Bool {
        !trimmedUsername.isEmpty && !password.trimmingCharacters(in: .whitespaces).isEmpty && !isSubmitting
    }

    private var visibleError: String? { isErrorDismissed ? nil : errorMessage }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    AuthHeader(title: "Welcome back", description: "Sign in to continue to your Sverto server.", systemImage: "key.fill")
                }
                Section {
                    HStack(spacing: 10) {
                        Image(systemName: "person")
                            .foregroundStyle(.secondary)
                            .accessibilityHidden(true)
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Username")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            TextField("Username", text: $username)
                                .textContentType(.username)
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .submitLabel(.next)
                                .focused($focusedField, equals: .username)
                                .onSubmit { focusedField = .password }
                        }
                    }
                    HStack(spacing: 10) {
                        Image(systemName: "lock")
                            .foregroundStyle(.secondary)
                            .accessibilityHidden(true)
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Password")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            Group {
                                if isPasswordVisible {
                                    TextField("Password", text: $password)
                                } else {
                                    SecureField("Password", text: $password)
                                }
                            }
                            .textContentType(.password)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .submitLabel(.done)
                            .focused($focusedField, equals: .password)
                            .onSubmit(submit)
                        }
                        Button {
                            togglePasswordVisibility()
                        } label: {
                            Image(systemName: isPasswordVisible ? "eye.slash" : "eye")
                                .foregroundStyle(.secondary)
                                .frame(minWidth: 44, minHeight: 44)
                                .contentShape(.rect)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel(isPasswordVisible ? "Hide password" : "Show password")
                    }
                }
                .disabled(isSubmitting)
                if let visibleError {
                    Section {
                        FormErrorRow(message: visibleError)
                    }
                }
            }
            .scrollDismissesKeyboard(.interactively)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Back") { session.showWelcome() }
                        .disabled(isSubmitting)
                }
            }
            .safeAreaInset(edge: .bottom) {
                VStack(spacing: 12) {
                    ProminentActionButton(title: "Sign in", isBusy: isSubmitting, action: submit)
                        .disabled(!canSubmit)
                    Button("Don't have an account? Create one") { session.showSignUp() }
                        .frame(minHeight: 44)
                        .multilineTextAlignment(.center)
                        .disabled(isSubmitting)
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 12)
            }
            .onChange(of: username) { isErrorDismissed = true }
            .onChange(of: password) { isErrorDismissed = true }
            .onChange(of: errorMessage) {
                isErrorDismissed = false
                if errorMessage != nil { Haptics.error() }
            }
        }
    }

    private func submit() {
        guard canSubmit else { return }
        focusedField = nil
        session.signIn(username: trimmedUsername, password: password)
    }

    private func togglePasswordVisibility() {
        let keepsFocus = focusedField == .password
        isPasswordVisible.toggle()
        if keepsFocus {
            Task { focusedField = .password }
        }
    }
}
