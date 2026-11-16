import SwiftUI
import UIKit

// The pieces every screen is made of: the page, cards and rows, the search
// pill, a row's menu, buttons. Sizes follow the phone mockup: cards with a
// radius of 20, a 52-high search pill, 16 at the sides.

extension LinkMood {
    /// Green connected, amber trying, grey before pairing, red when the
    /// pairing ended. Never the accent colour.
    var color: Color {
        switch self {
        case .off: return Theme.faint
        case .connected: return Theme.ok
        case .trying: return Theme.warn
        case .ended: return Theme.bad
        }
    }
}

struct StatusDot: View {
    let color: Color

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: 8, height: 8)
            .accessibilityHidden(true)
    }
}

/// A tab's page: its title, then whatever it lists, scrolling together.
struct Page<Content: View>: View {
    private let title: String
    private let bottom: CGFloat
    private let content: Content

    /// `bottom` leaves room under the list, for a floating button.
    init(_ title: String, bottom: CGFloat = 24, @ViewBuilder content: () -> Content) {
        self.title = title
        self.bottom = bottom
        self.content = content()
    }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                Text(title)
                    .pageTitle()
                    .foregroundStyle(Theme.text)
                    .padding(.top, 20)
                    .padding(.bottom, 16)
                    .accessibilityAddTraits(.isHeader)
                content
            }
            .padding(.horizontal, 16)
            .padding(.bottom, bottom)
        }
        .scrollDismissesKeyboard(.immediately)
        .background(Theme.background)
    }
}

/// Rows on one rounded surface.
struct Card<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.card)
        .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
    }
}

/// The line between rows in a card.
struct Hairline: View {
    var inset: CGFloat = 16

    var body: some View {
        Rectangle()
            .fill(Theme.hairline)
            .frame(height: 1)
            .padding(.leading, inset)
            .padding(.trailing, 16)
    }
}

/// "Today", "Your PC": the small teal line over a group.
struct GroupHeader: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .labelText(600)
            .foregroundStyle(Theme.accent)
            .padding(.leading, 8)
            .padding(.top, 18)
            .padding(.bottom, 8)
            .accessibilityAddTraits(.isHeader)
    }
}

/// The round mark at the start of a row.
struct RowMark: View {
    let icon: String

    var body: some View {
        Image(systemName: icon)
            .font(.system(size: 16, weight: .medium))
            .foregroundStyle(Theme.accent)
            .frame(width: 36, height: 36)
            .background(Theme.pill.opacity(0.6), in: Circle())
            .accessibilityHidden(true)
    }
}

struct Chevron: View {
    var body: some View {
        Image(systemName: "chevron.right")
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(Theme.faint)
            .accessibilityHidden(true)
    }
}

/// What a list says when it has nothing to show.
struct EmptyState: View {
    let icon: String
    let text: String

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: icon)
                .font(.system(size: 26))
                .foregroundStyle(Theme.faint)
                .accessibilityHidden(true)
            Text(text)
                .smallText()
                .foregroundStyle(Theme.faint)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 24)
        .padding(.vertical, 48)
    }
}

/// The search pill: a filled field with no outline until it has the cursor.
struct SearchField: View {
    @Binding var text: String
    let placeholder: String
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 17))
                .foregroundStyle(focused ? Theme.secondary : Theme.faint)
                .accessibilityHidden(true)
            TextField("", text: $text, prompt: Text(placeholder).foregroundStyle(Theme.faint))
                .bodyText()
                .foregroundStyle(Theme.text)
                .focused($focused)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .submitLabel(.search)
                .accessibilityLabel(placeholder)
            if !text.isEmpty {
                Button {
                    text = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 17))
                        .foregroundStyle(Theme.faint)
                        .frame(width: 44, height: 44)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Clear search")
            }
        }
        .padding(.leading, 16)
        .padding(.trailing, text.isEmpty ? 16 : 2)
        .frame(height: 52)
        .background(focused ? Theme.card : Theme.field, in: Capsule())
        .overlay(Capsule().strokeBorder(Theme.accent, lineWidth: focused ? 2 : 0))
        .contentShape(Capsule())
        .onTapGesture { focused = true }
    }
}

/// `text` with the first place `query` is found picked out, for search results.
func highlighted(_ text: String, _ query: String) -> AttributedString {
    var marked = AttributedString(text)
    let query = query.trimmingCharacters(in: .whitespaces)
    guard !query.isEmpty, let found = marked.range(of: query, options: .caseInsensitive) else {
        return marked
    }
    marked[found].foregroundColor = Theme.accent
    marked[found].backgroundColor = Theme.pill
    return marked
}

// MARK: - Buttons

/// A row that answers a touch by darkening a little.
struct RowPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .contentShape(Rectangle())
            .background(configuration.isPressed ? Theme.tabBar : Color.clear)
    }
}

/// The filled teal button.
struct FilledButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        Face(configuration: configuration)
    }

    private struct Face: View {
        let configuration: ButtonStyleConfiguration
        @Environment(\.isEnabled) private var enabled

        var body: some View {
            configuration.label
                .bodyText(600)
                .foregroundStyle(Color.white)
                .frame(maxWidth: .infinity, minHeight: 48)
                .background(Theme.fill.opacity(enabled ? 1 : 0.4), in: Capsule())
                .scaleEffect(configuration.isPressed ? 0.97 : 1)
                .animation(Theme.spring, value: configuration.isPressed)
        }
    }
}

/// A quiet button on a card: text in the accent colour, or a soft fill.
struct SoftButton: ButtonStyle {
    var filled = true

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .labelText(600)
            .foregroundStyle(filled ? Theme.text : Theme.accent)
            .frame(maxWidth: .infinity, minHeight: 44)
            .background(filled ? Theme.card : Color.clear, in: Capsule())
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(Theme.spring, value: configuration.isPressed)
    }
}

/// The floating button at the bottom of a list: Add word, New note.
struct FloatingButton: View {
    let label: String
    let icon: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.system(size: 17, weight: .semibold))
                    .accessibilityHidden(true)
                Text(label)
                    .bodyText(600)
            }
            .foregroundStyle(Color.white)
            .padding(.leading, 16)
            .padding(.trailing, 20)
            .frame(height: 56)
            .background(Theme.fill, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .shadow(color: Color.black.opacity(0.16), radius: 12, y: 6)
        }
        .buttonStyle(Pressed())
        .padding(16)
    }

    private struct Pressed: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .scaleEffect(configuration.isPressed ? 0.96 : 1)
                .animation(Theme.spring, value: configuration.isPressed)
        }
    }
}

// MARK: - A row's menu

/// One choice in a row's menu.
struct MenuAction: Identifiable {
    let id = UUID()
    let label: String
    let icon: String
    var danger = false
    let run: () -> Void
}

/// A row's menu: what the row is, then what can be done with it.
struct MenuModel: Identifiable {
    let id = UUID()
    let title: String
    var subtitle: String?
    let actions: [MenuAction]
}

/// The menu as a sheet from the bottom. Delete is always in here, never a
/// single tap on the row.
private struct MenuSheet: View {
    let model: MenuModel
    let pick: (MenuAction) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(model.title)
                    .titleText()
                    .foregroundStyle(Theme.text)
                    .lineLimit(2)
                    .padding(.horizontal, 4)
                if let subtitle = model.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .smallText()
                        .foregroundStyle(Theme.faint)
                        .lineLimit(2)
                        .padding(.horizontal, 4)
                        .padding(.top, 2)
                }
                Spacer().frame(height: 10)
                ForEach(model.actions) { action in
                    Button {
                        pick(action)
                    } label: {
                        HStack(spacing: 16) {
                            Image(systemName: action.icon)
                                .font(.system(size: 18))
                                .frame(width: 24)
                                .foregroundStyle(action.danger ? Theme.bad : Theme.secondary)
                                .accessibilityHidden(true)
                            Text(action.label)
                                .bodyText()
                                .foregroundStyle(action.danger ? Theme.bad : Theme.text)
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 12)
                        .frame(minHeight: 56)
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .buttonStyle(MenuPress())
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 28)
            .padding(.bottom, 16)
        }
        .scrollBounceBehavior(.basedOnSize)
        .presentationDetents([.height(height)])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(Theme.card)
    }

    /// About what the menu needs: its title on one line or two, the line
    /// under it, and 56 for each action.
    private var height: CGFloat {
        let title: CGFloat = model.title.count > 34 ? 48 : 24
        let subtitle: CGFloat = (model.subtitle ?? "").isEmpty ? 0 : 22
        return 28 + title + subtitle + 10 + CGFloat(56 * model.actions.count) + 16
    }

    private struct MenuPress: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .contentShape(Rectangle())
                .background(
                    configuration.isPressed ? Theme.field : Color.clear,
                    in: RoundedRectangle(cornerRadius: 14, style: .continuous)
                )
        }
    }
}

/// Shows a menu while `menu` is set. The picked action runs once the sheet
/// has gone, so it can open a sheet or an alert of its own.
private struct MenuHost: ViewModifier {
    @Binding var menu: MenuModel?
    @State private var picked: (() -> Void)?

    func body(content: Content) -> some View {
        content.sheet(item: $menu, onDismiss: {
            let run = picked
            picked = nil
            run?()
        }, content: { model in
            MenuSheet(model: model) { action in
                picked = action.run
                menu = nil
            }
        })
    }
}

extension View {
    func menuSheet(_ menu: Binding<MenuModel?>) -> some View {
        modifier(MenuHost(menu: menu))
    }

    /// A form in a sheet, in Legato's colours.
    func formSheet() -> some View {
        presentationDetents([.large])
            .presentationDragIndicator(.visible)
            .presentationCornerRadius(28)
            .presentationBackground(Theme.card)
    }
}

// MARK: - Forms

/// A labelled field in a form sheet, with a line of help when it needs one.
struct FormField: View {
    let label: String
    @Binding var text: String
    var help: String?
    var lines = 1

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            // The field itself carries the label for VoiceOver.
            Text(label)
                .smallText(500)
                .foregroundStyle(Theme.secondary)
                .accessibilityHidden(true)
            TextField("", text: $text, axis: lines > 1 ? .vertical : .horizontal)
                .bodyText()
                .foregroundStyle(Theme.text)
                .lineLimit(lines > 1 ? lines...8 : 1...1)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .frame(minHeight: 48)
                .background(Theme.background, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .strokeBorder(Theme.outline, lineWidth: 1)
                )
                .accessibilityLabel(label)
            if let help {
                Text(help)
                    .smallText()
                    .foregroundStyle(Theme.faint)
            }
        }
    }
}

/// Lays chips out in rows, wrapping to the next when one is full.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let limit = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? .greatestFiniteMagnitude
        var x: CGFloat = 0
        var y: CGFloat = 0
        var row: CGFloat = 0
        var widest: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0, x + size.width > limit {
                x = 0
                y += row + spacing
                row = 0
            }
            x += size.width
            widest = max(widest, x)
            x += spacing
            row = max(row, size.height)
        }
        return CGSize(width: widest, height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX
        var y = bounds.minY
        var row: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX {
                x = bounds.minX
                y += row + spacing
                row = 0
            }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}

// MARK: - Settings rows

/// A settings row: a title, one line of help, and whatever sits at the end.
struct SettingRow<Trailing: View>: View {
    private let title: String
    private let detail: String?
    private let action: (() -> Void)?
    private let trailing: Trailing

    init(_ title: String, detail: String? = nil, action: (() -> Void)? = nil, @ViewBuilder trailing: () -> Trailing) {
        self.title = title
        self.detail = detail
        self.action = action
        self.trailing = trailing()
    }

    var body: some View {
        if let action {
            Button(action: action) { row }
                .buttonStyle(RowPress())
        } else {
            row
        }
    }

    private var row: some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .bodyText()
                    .foregroundStyle(Theme.text)
                if let detail {
                    Text(detail)
                        .smallText()
                        .foregroundStyle(Theme.faint)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .frame(minHeight: 64)
    }
}

extension SettingRow where Trailing == EmptyView {
    init(_ title: String, detail: String? = nil, action: (() -> Void)? = nil) {
        self.init(title, detail: detail, action: action) { EmptyView() }
    }
}

// MARK: - Copy and share

@MainActor
func copyText(_ text: String) {
    UIPasteboard.general.string = text
}

/// Text on its way to the system's share sheet.
struct SharedText: Identifiable {
    let id = UUID()
    let text: String
}

/// The system's share sheet for a piece of text.
struct ShareSheet: UIViewControllerRepresentable {
    let text: String

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [text], applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}

// MARK: - The toast

struct ToastView: View {
    let toast: Toast
    let tapped: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Text(toast.text)
                .labelText(400)
                .foregroundStyle(Theme.toastText)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let label = toast.actionLabel {
                Button(action: tapped) {
                    Text(label)
                        .labelText(600)
                        .foregroundStyle(Theme.toastAction)
                        .padding(.horizontal, 10)
                        .frame(minHeight: 44)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.leading, 16)
        .padding(.trailing, toast.actionLabel == nil ? 16 : 6)
        .frame(minHeight: 48)
        .background(Theme.toast, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .shadow(color: Color.black.opacity(0.18), radius: 12, y: 4)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("toast")
    }
}
