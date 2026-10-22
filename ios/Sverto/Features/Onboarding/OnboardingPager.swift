import SwiftUI

struct OnboardingPager: View {
    let model: OnboardingViewModel
    let onComplete: () -> Void

    @State private var page = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var pages: [OnboardingStepId] { model.steps.filter { $0 != .welcome } }
    private var includesWelcome: Bool { model.steps.contains(.welcome) }
    private var isLastPage: Bool { page == pages.count - 1 }

    private var isForwardEnabled: Bool {
        guard pages.indices.contains(page) else { return false }
        return pages[page] != .baseCurrency || model.selectedCurrency != nil
    }

    var body: some View {
        if pages.isEmpty {
            LoadingScreen()
                .task { onComplete() }
        } else {
            content
        }
    }

    private var content: some View {
        VStack(spacing: 0) {
            OnboardingPageIndicator(pageCount: model.steps.count, currentPage: page + (includesWelcome ? 1 : 0))
                .padding(.top, 24)
                .padding(.bottom, 8)
            TabView(selection: $page) {
                ForEach(pages.indices, id: \.self) { index in
                    pageView(for: pages[index])
                        .tag(index)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            if let error = model.error {
                Text(error)
                    .font(.footnote)
                    .foregroundStyle(.red)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 24)
            }
            if pages.indices.contains(page), pages[page] == .aiFeatures {
                AiFeaturesChoiceBar(onChoose: chooseAiFeatures)
            } else {
                OnboardingNavBar(
                    isFirstPage: page == 0,
                    isLastPage: isLastPage,
                    isForwardEnabled: isForwardEnabled,
                    onBack: { withAnimation(reduceMotion ? nil : .default) { page -= 1 } },
                    onNext: { withAnimation(reduceMotion ? nil : .default) { page += 1 } },
                    onFinish: { Task { await model.finish() } }
                )
            }
        }
        .background(Color(.systemGroupedBackground))
        .disabled(model.isSaving)
        .overlay {
            if model.isSaving {
                ZStack {
                    Color.black.opacity(0.32)
                        .ignoresSafeArea()
                    ProgressView()
                        .controlSize(.large)
                }
            }
        }
        .task {
            if pages.contains(.baseCurrency) { await model.loadCurrencies() }
        }
        .onChange(of: page) { model.clearError() }
        .onChange(of: model.finished) { _, finished in
            if finished { onComplete() }
        }
    }

    @ViewBuilder
    private func pageView(for step: OnboardingStepId) -> some View {
        switch step {
        case .welcome:
            EmptyView()
        case .baseCurrency:
            BaseCurrencyPage(model: model)
        case .aiFeatures:
            AiFeaturesPage()
        case .notifications:
            NotificationsPage()
        }
    }

    private func chooseAiFeatures(_ enabled: Bool) {
        Task {
            guard await model.setAiFeaturesEnabled(enabled) else { return }
            if isLastPage {
                await model.finish()
            } else {
                withAnimation(reduceMotion ? nil : .default) { page += 1 }
            }
        }
    }
}
