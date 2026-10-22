import Testing
@testable import Sverto

struct OnboardingStepsTests {
    @Test func currentVersionIsTwo() {
        #expect(OnboardingSteps.currentVersion == 2)
    }

    @Test func freshUserSeesEveryStepInDeclaredOrder() {
        #expect(OnboardingSteps.steps(forCompletedVersion: 0) == [.welcome, .baseCurrency, .aiFeatures, .notifications])
    }

    @Test func userWhoCompletedVersionOneSeesOnlyTheAiFeaturesStep() {
        #expect(OnboardingSteps.steps(forCompletedVersion: 1) == [.aiFeatures])
    }

    @Test func completedCurrentVersionHasNothingLeft() {
        #expect(OnboardingSteps.steps(forCompletedVersion: OnboardingSteps.currentVersion).isEmpty)
    }

    @Test func newerThanCurrentVersionHasNothingLeft() {
        #expect(OnboardingSteps.steps(forCompletedVersion: 99).isEmpty)
    }

    @Test func negativeVersionBehavesLikeFreshUser() {
        #expect(OnboardingSteps.steps(forCompletedVersion: -1) == OnboardingStepId.allCases)
    }

    @Test func everyStepWasIntroducedAtOrBeforeCurrentVersion() {
        for step in OnboardingStepId.allCases {
            #expect(step.introducedIn >= 1)
            #expect(step.introducedIn <= OnboardingSteps.currentVersion)
        }
    }

    @Test func stepsAreOnlyThoseIntroducedAfterCompletedVersion() {
        for version in Int32(-1)...OnboardingSteps.currentVersion {
            let steps = OnboardingSteps.steps(forCompletedVersion: version)
            #expect(steps.allSatisfy { $0.introducedIn > version })
            #expect(steps == OnboardingStepId.allCases.filter { $0.introducedIn > version })
        }
    }
}
