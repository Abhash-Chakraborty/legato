import LegatoCore
import SwiftUI

/// Dictations from both devices, newest first, with the PC's ones marked.
struct HomeScreen: View {
    @EnvironmentObject private var store: Store
    let openSettings: () -> Void
    let openPair: () -> Void

    @State private var query = ""
    @State private var menu: MenuModel?
    @State private var sharing: SharedText?
    @State private var adding: WordDraft?

    private var shown: [HistoryEntry] {
        let q = query.trimmingCharacters(in: .whitespaces)
        if q.isEmpty { return store.history }
        return store.history.filter {
            $0.text.localizedCaseInsensitiveContains(q) || $0.app.localizedCaseInsensitiveContains(q)
        }
    }

    var body: some View {
        Page("Home") {
            if store.state.paired {
                SyncChip(status: pcStatus(store.state, device: store.device), action: openSettings)
            } else {
                PairCard(action: openPair)
            }
            SearchField(text: $query, placeholder: "Search your dictations")
                .padding(.top, 14)

            let entries = shown
            if entries.isEmpty {
                if store.history.isEmpty {
                    EmptyState(icon: "mic", text: "Your dictations show up here.")
                } else {
                    EmptyState(icon: "magnifyingglass", text: "Nothing matches “\(query)”.")
                }
            }
            ForEach(byDay(entries, at: { $0.at })) { day in
                GroupHeader(day.label)
                Card {
                    ForEach(day.items, id: \.id) { entry in
                        if entry.id != day.items.first?.id {
                            Hairline(inset: 66)
                        }
                        row(entry)
                    }
                }
            }
        }
        .menuSheet($menu)
        .sheet(item: $sharing) { shared in
            ShareSheet(text: shared.text)
                .presentationDetents([.medium, .large])
        }
        .sheet(item: $adding) { draft in
            AddWordSheet(draft: draft)
                .environmentObject(store)
        }
    }

    private func meta(_ entry: HistoryEntry, cancelled: Bool = true) -> String {
        dotted([
            entry.app,
            timeOf(entry.at),
            entry.device.map { "On \($0)" },
            cancelled && entry.cancelled ? "Cancelled" : nil,
        ])
    }

    /// A dictation: tap to copy it, the dots for everything else.
    private func row(_ entry: HistoryEntry) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Button {
                copyText(entry.text)
                store.show("Copied")
            } label: {
                HStack(alignment: .top, spacing: 14) {
                    RowMark(icon: mark(entry))
                    VStack(alignment: .leading, spacing: 4) {
                        Text(highlighted(entry.text, query))
                            .bodyText()
                            .foregroundStyle(Theme.text)
                            .lineLimit(3)
                            .multilineTextAlignment(.leading)
                        Text(meta(entry))
                            .smallText()
                            .foregroundStyle(Theme.faint)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.leading, 16)
                .padding(.vertical, 14)
            }
            .buttonStyle(RowPress())
            .accessibilityHint("Copies it")

            Button {
                menu = choices(for: entry)
            } label: {
                Image(systemName: "ellipsis")
                    .font(.system(size: 17, weight: .medium))
                    .foregroundStyle(Theme.faint)
                    .frame(width: 44, height: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .padding(.top, 8)
            .padding(.trailing, 6)
            .accessibilityLabel("More for this dictation")
        }
    }

    /// A phone for a dictation made here, a computer for the PC's, another
    /// phone for one that reached this phone through the PC.
    private func mark(_ entry: HistoryEntry) -> String {
        guard let device = entry.device else { return "mic.fill" }
        if let pc = store.state.pcName, device != pc { return "iphone" }
        return "desktopcomputer"
    }

    private func choices(for entry: HistoryEntry) -> MenuModel {
        MenuModel(
            title: entry.text,
            subtitle: meta(entry, cancelled: false),
            actions: [
                MenuAction(label: "Copy", icon: "doc.on.doc") {
                    copyText(entry.text)
                    store.show("Copied")
                },
                MenuAction(label: "Share", icon: "square.and.arrow.up") {
                    sharing = SharedText(text: entry.text)
                },
                MenuAction(label: "Add a word from this", icon: "book") {
                    adding = WordDraft(source: entry.text)
                },
                MenuAction(label: "Delete", icon: "trash", danger: true) {
                    store.run { try $0.deleteHistory(id: entry.id) }
                    // Back where it was, with the time it had.
                    store.show("Dictation deleted", action: "Undo", perform: {
                        store.run {
                            try $0.restoreHistory(
                                text: entry.text, app: entry.app, at: entry.at, cancelled: entry.cancelled
                            )
                        }
                    })
                },
            ]
        )
    }
}

/// The PC's name and how it stands, as a pill that opens Settings.
private struct SyncChip: View {
    let status: PcStatus
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                StatusDot(color: status.mood.color)
                Text(status.text)
                    .smallText()
                    .foregroundStyle(Theme.secondary)
                    .lineLimit(1)
                Chevron()
            }
            .padding(.leading, 12)
            .padding(.trailing, 10)
            .frame(minHeight: 34)
            .background(Theme.card, in: Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("sync-chip")
    }
}

/// Before pairing: where to start.
private struct PairCard: View {
    let action: () -> Void

    var body: some View {
        Card {
            Button(action: action) {
                HStack(spacing: 14) {
                    RowMark(icon: "desktopcomputer")
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Pair with your PC")
                            .bodyText()
                            .foregroundStyle(Theme.text)
                        Text("Get your Dictionary, snippets and history from Legato on Windows.")
                            .smallText()
                            .foregroundStyle(Theme.faint)
                            .multilineTextAlignment(.leading)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Chevron()
                }
                .padding(16)
            }
            .buttonStyle(RowPress())
            .accessibilityIdentifier("pair-card")
        }
    }
}
