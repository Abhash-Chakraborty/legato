import LegatoCore
import SwiftUI

/// Scratchpad notes from both devices, the last edited first.
struct ScratchpadScreen: View {
    @EnvironmentObject private var store: Store
    /// Opens a note; nil starts a new one.
    let open: (Int64?) -> Void

    @State private var menu: MenuModel?
    @State private var sharing: SharedText?

    var body: some View {
        Page("Scratchpad", bottom: 104) {
            if store.scratch.isEmpty {
                EmptyState(icon: "note.text", text: "Notes you type here sync with the Scratchpad on your PC.")
            } else {
                Card {
                    ForEach(store.scratch, id: \.id) { note in
                        if note.id != store.scratch.first?.id {
                            Hairline()
                        }
                        row(note)
                    }
                }
            }
        }
        .overlay(alignment: .bottomTrailing) {
            FloatingButton(label: "New note", icon: "square.and.pencil") {
                open(nil)
            }
        }
        .menuSheet($menu)
        .sheet(item: $sharing) { shared in
            ShareSheet(text: shared.text)
                .presentationDetents([.medium, .large])
        }
    }

    private func heading(_ note: ScratchNote) -> String {
        scratchHeading(title: note.title, text: note.text)
    }

    /// A note: tap to open it, the dots for everything else.
    private func row(_ note: ScratchNote) -> some View {
        HStack(alignment: .center, spacing: 0) {
            Button {
                open(note.id)
            } label: {
                VStack(alignment: .leading, spacing: 3) {
                    Text(heading(note))
                        .bodyText()
                        .foregroundStyle(Theme.text)
                        .lineLimit(1)
                    Text(dotted([ago(note.updatedAt), scratchPreview(title: note.title, text: note.text)]))
                        .smallText()
                        .foregroundStyle(Theme.faint)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 16)
                .padding(.vertical, 12)
                .frame(minHeight: 60)
            }
            .buttonStyle(RowPress())
            .accessibilityIdentifier("scratch-\(heading(note))")

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
            .padding(.trailing, 6)
            .accessibilityLabel("More for this note")
        }
    }

    private func choices(for note: ScratchNote) -> MenuModel {
        MenuModel(
            title: heading(note),
            subtitle: ago(note.updatedAt),
            actions: [
                MenuAction(label: "Open", icon: "pencil") {
                    open(note.id)
                },
                MenuAction(label: "Copy", icon: "doc.on.doc") {
                    copyText(note.text)
                    store.show("Copied")
                },
                MenuAction(label: "Share", icon: "square.and.arrow.up") {
                    sharing = SharedText(text: note.text)
                },
                MenuAction(label: "Delete", icon: "trash", danger: true) {
                    store.run { try $0.deleteScratch(id: note.id) }
                    store.show("Deleted \(heading(note))", action: "Undo", perform: {
                        store.run { _ = try $0.saveScratch(id: nil, title: note.title, text: note.text) }
                    })
                },
            ]
        )
    }
}

/// Saves one note in order, so a quick second save can't make a second
/// note. What it remembers is touched only inside the store's edits, which
/// run one at a time.
private final class NoteSaver {
    private let store: Store
    private var id: Int64?
    private var last: (title: String, text: String)?

    init(store: Store, id: Int64?, title: String, text: String) {
        self.store = store
        self.id = id
        if id != nil { last = (title, text) }
    }

    func save(title: String, text: String) {
        store.run { [self] core in
            if let last, last.title == title, last.text == text { return }
            if id == nil, NoteSaver.blank(title), NoteSaver.blank(text) { return }
            id = try core.saveScratch(id: id, title: title, text: text)
            last = (title, text)
        }
    }

    /// The last save on the way out; a note left empty goes, as on the PC.
    func close(title: String, text: String) {
        if !NoteSaver.blank(title) || !NoteSaver.blank(text) {
            save(title: title, text: text)
            return
        }
        store.run { [self] core in
            guard let gone = id else { return }
            try core.deleteScratch(id: gone)
            id = nil
            last = nil
        }
    }

    private static func blank(_ text: String) -> Bool {
        text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// One Scratchpad note: a title and the text. It saves a moment after you
/// stop typing, and once more on the way out.
struct ScratchEditor: View {
    @EnvironmentObject private var store: Store
    /// The note to open; nil is a new one.
    let id: Int64?

    private enum Field: Hashable {
        case title, text
    }

    @State private var title = ""
    @State private var text = ""
    @State private var saver: NoteSaver?
    @State private var pending: DispatchWorkItem?
    @FocusState private var focus: Field?

    init(id: Int64?) {
        self.id = id
    }

    private var blank: Bool {
        text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TextField("", text: $title, prompt: Text("Title").foregroundStyle(Theme.faint))
                .modifier(LegatoText(size: 22, weight: 600, relativeTo: .title2))
                .foregroundStyle(Theme.text)
                .focused($focus, equals: .title)
                .submitLabel(.next)
                .onSubmit { focus = .text }
                .padding(.horizontal, 20)
                .padding(.top, 8)
                .padding(.bottom, 4)
                .accessibilityLabel("Title")
            ZStack(alignment: .topLeading) {
                if text.isEmpty {
                    Text("Type here.")
                        .bodyText()
                        .foregroundStyle(Theme.faint)
                        .padding(.horizontal, 20)
                        .padding(.top, 8)
                        .accessibilityHidden(true)
                }
                TextEditor(text: $text)
                    .bodyText()
                    .foregroundStyle(Theme.text)
                    .scrollContentBackground(.hidden)
                    .focused($focus, equals: .text)
                    .padding(.horizontal, 15)
                    .accessibilityLabel("Note")
            }
        }
        .background(Theme.background.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button {
                    copyText(text)
                    store.show("Copied")
                } label: {
                    Image(systemName: "doc.on.doc")
                }
                .disabled(blank)
                .accessibilityLabel("Copy")
                ShareLink(item: text) {
                    Image(systemName: "square.and.arrow.up")
                }
                .disabled(blank)
                .accessibilityLabel("Share")
            }
            ToolbarItemGroup(placement: .keyboard) {
                Spacer()
                Button("Done") {
                    focus = nil
                }
            }
        }
        .onAppear(perform: load)
        .onChange(of: title) { saveSoon() }
        .onChange(of: text) { saveSoon() }
        .onDisappear {
            pending?.cancel()
            saver?.close(title: title, text: text)
        }
    }

    private func load() {
        guard saver == nil else { return }
        if let id, let note = store.scratch.first(where: { $0.id == id }) {
            title = note.title
            text = note.text
            saver = NoteSaver(store: store, id: id, title: note.title, text: note.text)
        } else {
            saver = NoteSaver(store: store, id: nil, title: "", text: "")
            // A new note is for writing: the cursor waits in it, once the
            // screen has slid in.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                focus = .text
            }
        }
    }

    private func saveSoon() {
        pending?.cancel()
        let (saver, title, text) = (self.saver, self.title, self.text)
        let work = DispatchWorkItem {
            saver?.save(title: title, text: text)
        }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6, execute: work)
    }
}
