import SwiftUI

/// The five tabs, the same as on Android.
enum AppTab: String, CaseIterable, Identifiable {
    case home, words, scratchpad, notes, settings

    var id: String { rawValue }

    var label: String {
        switch self {
        case .home: return "Home"
        case .words: return "Words"
        case .scratchpad: return "Scratchpad"
        case .notes: return "Notes"
        case .settings: return "Settings"
        }
    }

    var icon: String {
        switch self {
        case .home: return "house"
        case .words: return "book"
        case .scratchpad: return "note.text"
        case .notes: return "waveform"
        case .settings: return "gearshape"
        }
    }

    /// The filled form, where the symbol has one, for the tab that's open.
    var iconOn: String {
        switch self {
        case .home: return "house.fill"
        case .words: return "book.fill"
        case .settings: return "gearshape.fill"
        case .scratchpad, .notes: return icon
        }
    }
}

/// A screen opened over the tabs.
enum Route: Hashable {
    /// A Scratchpad note; nil is a new one.
    case scratch(Int64?)
    case note(Int64)
    /// Pairing, with the link it was opened with when there was one.
    case pair(String?)
    case advanced
}

struct RootView: View {
    @EnvironmentObject private var store: Store
    @Environment(\.scenePhase) private var phase
    @State private var tab: AppTab = .home
    @State private var path: [Route] = []
    @State private var wentAway = true

    private let tick = Timer.publish(every: 30, on: .main, in: .common).autoconnect()

    var body: some View {
        NavigationStack(path: $path) {
            VStack(spacing: 0) {
                ZStack {
                    ForEach(AppTab.allCases) { one in
                        page(one)
                            .opacity(one == tab ? 1 : 0)
                            .allowsHitTesting(one == tab)
                            .accessibilityHidden(one != tab)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .animation(.easeOut(duration: 0.16), value: tab)
                TabBar(tab: $tab)
            }
            .background(Theme.background.ignoresSafeArea())
            // The page's own background under the status bar, so a list
            // scrolls out of sight there instead of behind the clock.
            .overlay(alignment: .top) {
                Color.clear
                    .frame(maxWidth: .infinity)
                    .frame(height: 0)
                    .background(Theme.background.ignoresSafeArea(edges: .top))
            }
            .ignoresSafeArea(.keyboard, edges: .bottom)
            .toolbar(.hidden, for: .navigationBar)
            .navigationDestination(for: Route.self) { route in
                destination(route)
            }
        }
        .tint(Theme.accent)
        .overlay(alignment: .bottom) {
            if let toast = store.toast {
                ToastView(toast: toast) { store.toastTapped() }
                    .padding(.horizontal, 16)
                    .padding(.bottom, toastLift)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .id(toast.id)
            }
        }
        .animation(.spring(response: 0.36, dampingFraction: 0.85), value: store.toast?.id)
        .onOpenURL { url in
            // legato://pair?…: shown first, and paired only when you say so.
            if url.scheme == "legato", url.host == "pair" {
                path = [.pair(url.absoluteString)]
            }
        }
        .onChange(of: phase) { _, now in
            switch now {
            case .active:
                if wentAway {
                    wentAway = false
                    store.foreground()
                }
            case .background:
                wentAway = true
                store.background()
            default:
                break
            }
        }
        .onAppear {
            if phase == .active, wentAway {
                wentAway = false
                store.foreground()
            }
        }
        .onReceive(tick) { _ in
            if phase == .active { store.refresh() }
        }
    }

    /// How far the toast sits above the bottom: over the tabs, and over the
    /// floating button on the pages that have one.
    private var toastLift: CGFloat {
        if !path.isEmpty { return 16 }
        return tab == .words || tab == .scratchpad ? 156 : 84
    }

    @ViewBuilder
    private func page(_ one: AppTab) -> some View {
        switch one {
        case .home:
            HomeScreen(openSettings: { tab = .settings }, openPair: { path.append(.pair(nil)) })
        case .words:
            WordsScreen()
        case .scratchpad:
            ScratchpadScreen(open: { id in path.append(.scratch(id)) })
        case .notes:
            NotesScreen(open: { id in path.append(.note(id)) })
        case .settings:
            SettingsScreen(openPair: { path.append(.pair(nil)) }, openAdvanced: { path.append(.advanced) })
        }
    }

    @ViewBuilder
    private func destination(_ route: Route) -> some View {
        switch route {
        case let .scratch(id):
            ScratchEditor(id: id)
        case let .note(id):
            NoteScreen(id: id)
        case let .pair(link):
            PairScreen(link: link)
        case .advanced:
            AdvancedScreen()
        }
    }
}

/// The tabs along the bottom: an icon in a pill that slides to the tab
/// you pick, and its name.
private struct TabBar: View {
    @Binding var tab: AppTab
    @Namespace private var pill

    var body: some View {
        HStack(spacing: 0) {
            ForEach(AppTab.allCases) { one in
                let on = one == tab
                Button {
                    withAnimation(Theme.spring) { tab = one }
                } label: {
                    VStack(spacing: 3) {
                        ZStack {
                            if on {
                                Capsule()
                                    .fill(Theme.pill)
                                    .matchedGeometryEffect(id: "pill", in: pill)
                            }
                            Image(systemName: on ? one.iconOn : one.icon)
                                .font(.system(size: 18, weight: .medium))
                                .foregroundStyle(on ? Theme.accent : Theme.faint)
                        }
                        .frame(width: 60, height: 32)
                        Text(one.label)
                            .modifier(LegatoText(size: 12, weight: on ? 600 : 500, relativeTo: .caption2))
                            .foregroundStyle(on ? Theme.text : Theme.faint)
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                    }
                    .frame(maxWidth: .infinity, minHeight: 56)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(one.label)
                .accessibilityIdentifier("tab-\(one.rawValue)")
                .accessibilityAddTraits(on ? [.isSelected] : [])
            }
        }
        .padding(.top, 8)
        .background(Theme.tabBar.ignoresSafeArea(edges: .bottom))
    }
}
