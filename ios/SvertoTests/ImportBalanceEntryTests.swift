import Foundation
import Testing
@testable import Sverto

@MainActor
struct ImportBalanceEntryTests {
    @Test func readsGroupingAndDecimalSeparatorsOfTheLocale() {
        let english = Locale(identifier: "en_GB")
        let german = Locale(identifier: "de_DE")

        #expect(BalanceEntryField.quantity(from: "1,234.56", locale: english) == "1234.56")
        #expect(BalanceEntryField.quantity(from: "1,234", locale: english) == "1234")
        #expect(BalanceEntryField.quantity(from: "1.234,56", locale: german) == "1234.56")
        #expect(BalanceEntryField.quantity(from: "1,234", locale: german) == "1.234")
        #expect(BalanceEntryField.quantity(from: " -20.5 ", locale: english) == "-20.5")
        #expect(BalanceEntryField.quantity(from: "1.234,56", locale: english) == nil)
    }
}
