import Foundation

enum AiConsentCopy {
    static let onboardingTitle = "AI features"
    static let popupTitle = "Turn on AI features?"
    static let intro = "Myra chat and receipt scanning use Google's Gemini AI. To use them, Sverto sends Google:"
    static let sharedData: [(systemImage: String, text: String)] = [
        ("bubble.left.and.text.bubble.right", "Your Myra messages and any images or receipts you attach"),
        ("list.bullet.rectangle", "Account, transaction, holding and category details Myra looks up to answer you"),
        ("magnifyingglass", "Your transaction descriptions, so Myra can search them"),
    ]
    static let footer = "Google does not use this data to improve its products. You can use the rest of Sverto without AI features."
    static let privacyPolicyTitle = "Privacy policy"
    static let privacyPolicyURL = URL(string: "https://sverto.com/privacy")!
    static let turnOn = "Turn on"
    static let onboardingDecline = "Keep off"
    static let popupDecline = "Not now"
    static let settingsTitle = "AI features"
    static let settingsDescription = "Myra chat and receipt scanning use Google's Gemini AI. When off, none of your data is sent to Gemini and these features are unavailable."
}
