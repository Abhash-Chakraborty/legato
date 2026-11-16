import LegatoCore
import SwiftUI

/// Meetings and voice memos: the PC's, after a sync.
struct NotesScreen: View {
    @EnvironmentObject private var store: Store
    let open: (Int64) -> Void

    @State private var menu: MenuModel?
    /// Deleted, while Undo is offered; the note goes once the offer does.
    @State private var hidden: Set<Int64> = []

    var body: some View {
        let shown = store.notes.filter { !hidden.contains($0.id) }
        Page("Notes") {
            if shown.isEmpty {
                EmptyState(icon: "waveform", text: "Meetings and memos from your PC show up here after a sync.")
            }
            ForEach(byDay(shown, at: { $0.startedAt })) { day in
                GroupHeader(day.label)
                Card {
                    ForEach(day.items, id: \.id) { note in
                        if note.id != day.items.first?.id {
                            Hairline(inset: 66)
                        }
                        row(note)
                    }
                }
            }
        }
        .menuSheet($menu)
    }

    private func meta(_ note: NoteRow) -> String {
        dotted([
            timeOf(note.startedAt),
            minutes(note.durationMs),
            note.device.map { "On \($0)" } ?? "On this \(store.device)",
            openActions(note.openActions),
        ])
    }

    private func mark(_ note: NoteRow) -> String {
        guard let device = note.device else { return "waveform" }
        if let pc = store.state.pcName, device != pc { return "iphone" }
        return "desktopcomputer"
    }

    /// A note: tap to open it, the dots for the rest.
    private func row(_ note: NoteRow) -> some View {
        HStack(alignment: .top, spacing: 0) {
            Button {
                open(note.id)
            } label: {
                HStack(alignment: .top, spacing: 14) {
                    RowMark(icon: mark(note))
                    VStack(alignment: .leading, spacing: 3) {
                        Text(note.title)
                            .bodyText()
                            .foregroundStyle(Theme.text)
                            .lineLimit(1)
                        Text(meta(note))
                            .smallText()
                            .foregroundStyle(Theme.faint)
                            .multilineTextAlignment(.leading)
                        if !note.summary.isEmpty {
                            Text(note.summary)
                                .smallText()
                                .foregroundStyle(Theme.secondary)
                                .lineLimit(2)
                                .multilineTextAlignment(.leading)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.leading, 16)
                .padding(.vertical, 14)
            }
            .buttonStyle(RowPress())
            .accessibilityIdentifier("note-\(note.title)")

            Button {
                menu = choices(for: note)
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
            .accessibilityLabel("More for this note")
        }
    }

    private func choices(for note: NoteRow) -> MenuModel {
        var actions = [
            MenuAction(label: "Open", icon: "doc.text") {
                open(note.id)
            },
        ]
        // A PC's note is deleted there, where Recently deleted keeps it a while.
        if note.device == nil {
            actions.append(MenuAction(label: "Delete", icon: "trash", danger: true) {
                hidden.insert(note.id)
                store.show(
                    "Deleted \(note.title)",
                    action: "Undo",
                    expired: { store.run { _ = try $0.deleteNote(id: note.id) } },
                    perform: { hidden.remove(note.id) }
                )
            })
        }
        return MenuModel(
            title: note.title,
            subtitle: note.device.map { "Made on \($0); delete it there." } ?? meta(note),
            actions: actions
        )
    }
}

/// One note: its summary, the action items to tick off, and the transcript.
struct NoteScreen: View {
    @EnvironmentObject private var store: Store
    let id: Int64

    @State private var detail: NoteDetail?
    @State private var missing = false
    /// Ticks show at once; the core catches up in the background.
    @State private var ticked: [Int64: Bool] = [:]

    var body: some View {
        ScrollView {
            if let detail {
                content(detail)
            } else if missing {
                EmptyState(icon: "waveform", text: "This note isn't on this \(store.device) any more.")
            }
        }
        .background(Theme.background.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    copyText((detail?.transcript ?? []).joined(separator: "\n"))
                    store.show("Transcript copied")
                } label: {
                    Image(systemName: "doc.on.doc")
                }
                .disabled((detail?.transcript ?? []).isEmpty)
                .accessibilityLabel("Copy transcript")
            }
        }
        .onAppear(perform: load)
        .onChange(of: store.version) { load() }
    }

    private func load() {
        store.ask({ try $0.note(id: id) }, then: { answer in
            switch answer {
            case let .success(found):
                detail = found
                missing = false
            case .failure:
                missing = detail == nil
            }
        })
    }

    private func meta(_ note: NoteRow) -> String {
        dotted([dateAndTime(note.startedAt), minutes(note.durationMs), openActions(note.openActions)])
    }

    private func content(_ detail: NoteDetail) -> some View {
        LazyVStack(alignment: .leading, spacing: 0) {
            Text(detail.note.title)
                .pageTitle()
                .foregroundStyle(Theme.text)
                .padding(.top, 8)
                .accessibilityAddTraits(.isHeader)
            Text(meta(detail.note))
                .smallText()
                .foregroundStyle(Theme.secondary)
                .padding(.top, 6)

            if !detail.note.summary.isEmpty {
                GroupHeader("Summary")
                Text(detail.note.summary)
                    .bodyText()
                    .foregroundStyle(Theme.text)
                    .padding(.horizontal, 8)
            }

            if !detail.actions.isEmpty {
                GroupHeader("Action items")
                Card {
                    ForEach(detail.actions, id: \.id) { action in
                        if action.id != detail.actions.first?.id {
                            Hairline(inset: 54)
                        }
                        actionRow(action)
                    }
                }
            }

            if !detail.transcript.isEmpty {
                GroupHeader("Transcript")
                ForEach(Array(detail.transcript.enumerated()), id: \.offset) { _, line in
                    transcriptRow(line)
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 32)
    }

    /// An action item: tap to tick it off or bring it back.
    private func actionRow(_ action: ActionItem) -> some View {
        let done = ticked[action.id] ?? action.done
        return Button {
            ticked[action.id] = !done
            store.run { try $0.setActionDone(id: action.id, done: !done) }
        } label: {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: done ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: 22))
                    .foregroundStyle(done ? Theme.accent : Theme.outline)
                    .frame(width: 24)
                Text(action.text)
                    .bodyText()
                    .strikethrough(done)
                    .foregroundStyle(done ? Theme.secondary : Theme.text)
                    .multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
        }
        .buttonStyle(RowPress())
        .accessibilityLabel(action.text)
        .accessibilityValue(done ? "Done" : "Not done")
        .accessibilityIdentifier("action-\(action.text)")
    }

    /// A line of the transcript: who said it, then what.
    private func transcriptRow(_ line: String) -> some View {
        var who = ""
        var said = line
        if let split = line.range(of: ": ") {
            who = String(line[..<split.lowerBound])
            said = String(line[split.upperBound...])
        }
        let name: String
        switch who {
        case "you": name = "You"
        case "them": name = "Them"
        default: name = who.prefix(1).uppercased() + who.dropFirst()
        }
        return HStack(alignment: .top, spacing: 8) {
            Text(name)
                .labelText(600)
                .foregroundStyle(who == "you" ? Theme.accent : Theme.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(width: 56, alignment: .leading)
                .padding(.top, 1)
            Text(said)
                .bodyText()
                .foregroundStyle(Theme.text)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
    }
}
