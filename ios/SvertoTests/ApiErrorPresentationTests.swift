import Foundation
import SvertoCore
import Testing
@testable import Sverto

private struct OpaqueError: Error {}

struct ApiErrorPresentationTests {
    @Test func reasonBearingCasesSurfaceTheirReason() {
        #expect(ApiError.Network(reason: "No route to host").friendlyMessage == "No route to host")
        #expect(ApiError.Server(reason: "Account not found", status: 404).friendlyMessage == "Account not found")
        #expect(ApiError.Timeout(reason: "Request timed out").friendlyMessage == "Request timed out")
        #expect(ApiError.Parse(reason: "Unexpected token").friendlyMessage == "Unexpected token")
    }

    @Test func emptyReasonFallsBackToGenericMessage() {
        #expect(ApiError.Network(reason: "").friendlyMessage == "Something went wrong")
        #expect(ApiError.Server(reason: "", status: 500).friendlyMessage == "Something went wrong")
        #expect(ApiError.Timeout(reason: "").friendlyMessage == "Something went wrong")
        #expect(ApiError.Parse(reason: "").friendlyMessage == "Something went wrong")
    }

    @Test func missingLocalCredentialAsksToSignInAgain() {
        #expect(ApiError.MissingLocalCredential.friendlyMessage == "Your saved session is no longer available. Please sign in again.")
    }

    @Test func aiFeaturesDisabledSaysTheFeaturesAreOff() {
        #expect(ApiError.AiFeaturesDisabled.friendlyMessage == "AI features are turned off.")
    }

    @Test func friendlyMessageNeverLeaksTheDebugDescription() {
        let error = ApiError.Server(reason: "Forbidden", status: 403)

        #expect(!error.friendlyMessage.contains("status"))
        #expect(!error.friendlyMessage.contains("403"))
    }

    @Test func genericErrorExtensionDelegatesToApiError() {
        let error: any Error = ApiError.Network(reason: "Offline")

        #expect(error.friendlyMessage == "Offline")
    }

    @Test func cancellationIsReportedAsCancelled() {
        let error: any Error = CancellationError()

        #expect(error.friendlyMessage == "Cancelled")
    }

    @Test func unknownErrorsUseGenericMessage() {
        let error: any Error = OpaqueError()

        #expect(error.friendlyMessage == "Something went wrong")
    }

    @Test func nsErrorsUseGenericMessage() {
        let error: any Error = NSError(domain: NSURLErrorDomain, code: NSURLErrorNotConnectedToInternet, userInfo: nil)

        #expect(error.friendlyMessage == "Something went wrong")
    }
}
