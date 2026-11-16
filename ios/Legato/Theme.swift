import CoreText
import SwiftUI
import UIKit

/// Legato's own look, the same tokens as the Windows app and Legato for
/// Android: warm neutrals, teal as the one accent, Figtree. Each colour has
/// a light and a dark value and follows the system's appearance.
enum Theme {
    static let background = pair(0xF3F2EE, 0x151615)
    static let card = pair(0xFCFCFB, 0x1D1E1D)
    static let tabBar = pair(0xF5F4F0, 0x242624)
    static let field = pair(0xEDEBE5, 0x2C2E2C)

    static let text = pair(0x1A1A1A, 0xF1F0EC)
    static let secondary = pair(0x4A4845, 0xC7C5BF)
    static let faint = pair(0x8A8580, 0x8E8B85)

    /// Text buttons, links, day headers.
    static let accent = pair(0x1A5C5C, 0x66B8AD)
    /// Filled buttons.
    static let fill = pair(0x1A5C5C, 0x1D6F6A)
    /// The tab pill and picked segments.
    static let pill = pair(0xCEEDEA, 0x22433E)

    static let outline = pair(0xD9D5CC, 0x3A3D3A)
    static let hairline = pair(0xEBE8E1, 0x2A2C2A)

    /// Status colours, only for states: connected, trying, failed.
    static let ok = pair(0x2F9E6E, 0x3DB57F)
    static let warn = pair(0xC98A1A, 0xD9A040)
    static let bad = pair(0xC7423D, 0xE0605A)

    /// The toast is dark in both appearances.
    static let toast = pair(0x2B2C2B, 0x2B2C2B)
    static let toastText = pair(0xF1F0EC, 0xF1F0EC)
    static let toastAction = pair(0x95C7BB, 0x95C7BB)

    /// Movement: a spring with a little overshoot. Colour changes don't use it.
    static let spring = Animation.spring(response: 0.42, dampingFraction: 0.7)

    private static func pair(_ light: UInt32, _ dark: UInt32) -> Color {
        Color(uiColor: UIColor { traits in
            traits.userInterfaceStyle == .dark ? UIColor(rgb: dark) : UIColor(rgb: light)
        })
    }
}

private extension UIColor {
    convenience init(rgb: UInt32) {
        self.init(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

// MARK: - Figtree

extension Theme {
    /// The font file is one variable font; its weight axis runs from 300 to 900.
    private static let weightAxis = 0x7767_6874 // 'wght'

    /// Whether Figtree is there: the app lists it in Info.plist, and a
    /// bundle that doesn't (the tests') registers the file itself, once.
    private static let fontReady: Bool = {
        if !UIFont.fontNames(forFamilyName: "Figtree").isEmpty { return true }
        for bundle in [Bundle.main] + Bundle.allBundles {
            if let url = bundle.url(forResource: "Figtree", withExtension: "ttf") {
                return CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            }
        }
        return false
    }()

    @discardableResult
    static func registerFont() -> Bool {
        fontReady
    }

    /// Figtree at `weight` on its axis (400 regular, 500 medium, 600 semibold,
    /// 700 bold); the system font if Figtree can't be loaded.
    static func uiFont(size: CGFloat, weight: CGFloat) -> UIFont {
        registerFont()
        let variation = UIFontDescriptor.AttributeName(rawValue: kCTFontVariationAttribute as String)
        let attributes: [UIFontDescriptor.AttributeName: Any] = [
            .family: "Figtree",
            variation: [NSNumber(value: weightAxis): NSNumber(value: Double(weight))],
        ]
        let font = UIFont(descriptor: UIFontDescriptor(fontAttributes: attributes), size: size)
        if font.familyName == "Figtree" { return font }
        let fallback: UIFont.Weight = weight >= 600 ? .semibold : (weight >= 500 ? .medium : .regular)
        return UIFont.systemFont(ofSize: size, weight: fallback)
    }

    static func font(_ size: CGFloat, _ weight: CGFloat = 400) -> Font {
        Font(uiFont(size: size, weight: weight) as CTFont)
    }
}

/// Text in Figtree at a size that grows with the system's text size.
struct LegatoText: ViewModifier {
    @ScaledMetric private var size: CGFloat
    private let weight: CGFloat
    private let tracking: CGFloat
    private let leading: CGFloat

    /// `tracking` is in em; `leading` is extra space between lines.
    init(
        size: CGFloat,
        weight: CGFloat = 400,
        relativeTo style: Font.TextStyle = .body,
        tracking: CGFloat = 0,
        leading: CGFloat = 0
    ) {
        _size = ScaledMetric(wrappedValue: size, relativeTo: style)
        self.weight = weight
        self.tracking = tracking
        self.leading = leading
    }

    func body(content: Content) -> some View {
        content
            .font(Theme.font(size, weight))
            .tracking(tracking * size)
            .lineSpacing(leading)
    }
}

extension View {
    /// The page title: 28, semibold, a little tight.
    func pageTitle() -> some View {
        modifier(LegatoText(size: 28, weight: 600, relativeTo: .largeTitle, tracking: -0.02))
    }

    /// A sheet's or a card's title: 18, semibold.
    func titleText() -> some View {
        modifier(LegatoText(size: 18, weight: 600, relativeTo: .title3))
    }

    /// Rows and running text: 15 on 21.
    func bodyText(_ weight: CGFloat = 400) -> some View {
        modifier(LegatoText(size: 15, weight: weight, relativeTo: .body, leading: 3))
    }

    /// The line under a row's title: 13 on 18.
    func smallText(_ weight: CGFloat = 400) -> some View {
        modifier(LegatoText(size: 13, weight: weight, relativeTo: .footnote, leading: 2))
    }

    /// Buttons and group headers: 14.
    func labelText(_ weight: CGFloat = 500) -> some View {
        modifier(LegatoText(size: 14, weight: weight, relativeTo: .subheadline))
    }
}
