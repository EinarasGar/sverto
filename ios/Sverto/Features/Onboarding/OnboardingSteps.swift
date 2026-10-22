import Foundation

enum OnboardingStepId: CaseIterable, Hashable {
    case welcome
    case baseCurrency
    case aiFeatures
    case notifications

    var introducedIn: Int32 {
        switch self {
        case .welcome, .baseCurrency, .notifications: 1
        case .aiFeatures: 2
        }
    }
}

enum OnboardingSteps {
    static let currentVersion: Int32 = 2

    static func steps(forCompletedVersion version: Int32) -> [OnboardingStepId] {
        OnboardingStepId.allCases.filter { $0.introducedIn > version }
    }
}
