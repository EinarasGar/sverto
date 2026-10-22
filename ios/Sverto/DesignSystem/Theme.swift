import SwiftUI

extension Color {
    nonisolated static let brand = Color.accentColor
    nonisolated static let onAccent = Color("OnAccent")
    nonisolated static let gain = Color("GainColor")
    nonisolated static let loss = Color("LossColor")
    nonisolated static let screenBackground = Color(.systemGroupedBackground)
    nonisolated static let cardFill = Color(.secondarySystemGroupedBackground)
}

extension ShapeStyle where Self == Color {
    nonisolated static var brand: Color { .brand }
    nonisolated static var gain: Color { .gain }
    nonisolated static var loss: Color { .loss }
    nonisolated static var screenBackground: Color { .screenBackground }
    nonisolated static var cardFill: Color { .cardFill }
}

enum Layout {
    nonisolated static let cardRadius: CGFloat = 16
    nonisolated static let cardPadding: CGFloat = 16
    nonisolated static let tileRadius: CGFloat = 14
    nonisolated static let rowSpacing: CGFloat = 12
    nonisolated static let rowGlyphSize: CGFloat = 40
    nonisolated static let iconTileOpacity: Double = 0.14
    nonisolated static let readableWidth: CGFloat = 560
}
