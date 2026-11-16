import LegatoCore
import SwiftUI

/// The Dictionary and snippets, shared with the PC.
struct WordsScreen: View {
    @EnvironmentObject private var store: Store

    /// The sheet that's open: adding a word, or a snippet being made or edited.
    private enum Sheet: Identifiable {
        case word(WordDraft)
        case snippet(SnippetDraft)

        var id: UUID {
            switch self {
            case let .word(draft): return draft.id
            case let .snippet(draft): return draft.id
            }
        }
    }

    @State private var part: WordsPart = .words
    @State private var query = ""
    @State private var menu: MenuModel?
    @State private var form: Sheet?

    init() {}

    var body: some View {
        let q = query.trimmingCharacters(in: .whitespaces)
        Page(part == .words ? "Dictionary" : "Snippets", bottom: 104) {
            Segments(
                part: $part,
                words: "Words · \(store.words.count)",
                snippets: "Snippets · \(store.snippets.count)"
            )
            SearchField(text: $query, placeholder: part == .words ? "Search words" : "Search snippets")
                .padding(.top, 14)
                .padding(.bottom, 14)
            if part == .words {
                words(matching: q)
            } else {
                snippets(matching: q)
            }
        }
        .overlay(alignment: .bottomTrailing) {
            FloatingButton(label: part == .words ? "Add word" : "Add snippet", icon: "plus") {
                form = part == .words ? .word(WordDraft()) : .snippet(SnippetDraft())
            }
        }
        .menuSheet($menu)
        .sheet(item: $form) { form in
            switch form {
            case let .word(draft):
                AddWordSheet(draft: draft)
                    .environmentObject(store)
            case let .snippet(draft):
                SnippetSheet(draft: draft)
                    .environmentObject(store)
            }
        }
    }

    // MARK: Words

    private func heardAs(_ word: WordRow) -> String {
        if word.heard.isEmpty { return "Spelled as written" }
        return "Heard as " + word.heard.map { "“\($0)”" }.joined(separator: ", ")
    }

    @ViewBuilder
    private func words(matching q: String) -> some View {
        let shown = q.isEmpty ? store.words : store.words.filter { word in
            word.text.localizedCaseInsensitiveContains(q)
                || word.heard.contains { $0.localizedCaseInsensitiveContains(q) }
        }
        if shown.isEmpty {
            if store.words.isEmpty {
                EmptyState(icon: "book", text: "Names and terms you add here are spelled right in every dictation.")
            } else {
                EmptyState(icon: "magnifyingglass", text: "No word matches “\(q)”.")
            }
        } else {
            Card {
                ForEach(shown, id: \.id) { word in
                    if word.id != shown.first?.id {
                        Hairline()
                    }
                    Button {
                        menu = choices(for: word)
                    } label: {
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(highlighted(word.text, q))
                                    .bodyText()
                                    .foregroundStyle(Theme.text)
                                Text(heardAs(word))
                                    .smallText()
                                    .foregroundStyle(Theme.faint)
                                    .lineLimit(2)
                                    .multilineTextAlignment(.leading)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            if word.pinned {
                                Image(systemName: "pin.fill")
                                    .font(.system(size: 13))
                                    .foregroundStyle(Theme.faint)
                                    .accessibilityLabel("Pinned")
                            }
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                        .frame(minHeight: 60)
                    }
                    .buttonStyle(RowPress())
                    .accessibilityIdentifier("word-\(word.text)")
                }
            }
        }
    }

    private func choices(for word: WordRow) -> MenuModel {
        MenuModel(
            title: word.text,
            subtitle: heardAs(word),
            actions: [
                MenuAction(label: "Add what it's heard as", icon: "plus") {
                    form = .word(WordDraft(word: word.text, fixed: true))
                },
                MenuAction(label: "Copy", icon: "doc.on.doc") {
                    copyText(word.text)
                    store.show("Copied")
                },
                MenuAction(label: "Remove from Dictionary", icon: "trash", danger: true) {
                    store.run { try $0.removeWord(text: word.text) }
                    store.show("Removed \(word.text)", action: "Undo", perform: {
                        store.run { core in
                            if word.heard.isEmpty {
                                try core.addWordHeard(text: word.text, heard: "")
                            }
                            for heard in word.heard {
                                try core.addWordHeard(text: word.text, heard: heard)
                            }
                        }
                    })
                },
            ]
        )
    }

    // MARK: Snippets

    @ViewBuilder
    private func snippets(matching q: String) -> some View {
        let shown = q.isEmpty ? store.snippets : store.snippets.filter {
            $0.trigger.localizedCaseInsensitiveContains(q) || $0.text.localizedCaseInsensitiveContains(q)
        }
        if shown.isEmpty {
            if store.snippets.isEmpty {
                EmptyState(icon: "plus", text: "Say a trigger phrase and Legato types the saved text.")
            } else {
                EmptyState(icon: "magnifyingglass", text: "No snippet matches “\(q)”.")
            }
        } else {
            Card {
                ForEach(shown, id: \.id) { snippet in
                    if snippet.id != shown.first?.id {
                        Hairline()
                    }
                    Button {
                        menu = choices(for: snippet)
                    } label: {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(highlighted("“\(snippet.trigger)”", q))
                                .bodyText()
                                .foregroundStyle(Theme.text)
                            Text(snippet.text)
                                .smallText()
                                .foregroundStyle(Theme.faint)
                                .lineLimit(2)
                                .multilineTextAlignment(.leading)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                        .frame(minHeight: 60)
                    }
                    .buttonStyle(RowPress())
                    .accessibilityIdentifier("snippet-\(snippet.trigger)")
                }
            }
        }
    }

    private func choices(for snippet: SnippetRow) -> MenuModel {
        MenuModel(
            title: "“\(snippet.trigger)”",
            subtitle: snippet.text,
            actions: [
                MenuAction(label: "Edit", icon: "pencil") {
                    form = .snippet(SnippetDraft(editing: snippet.id, trigger: snippet.trigger, text: snippet.text))
                },
                MenuAction(label: "Copy the text", icon: "doc.on.doc") {
                    copyText(snippet.text)
                    store.show("Copied")
                },
                MenuAction(label: "Remove snippet", icon: "trash", danger: true) {
                    store.run { try $0.removeSnippet(id: snippet.id) }
                    store.show("Removed “\(snippet.trigger)”", action: "Undo", perform: {
                        store.run { try $0.saveSnippet(id: nil, trigger: snippet.trigger, text: snippet.text) }
                    })
                },
            ]
        )
    }
}

/// Which half of the page shows.
private enum WordsPart: Hashable {
    case words, snippets
}

/// Words or Snippets: a pill that slides to the one you pick.
private struct Segments: View {
    @Binding var part: WordsPart
    let words: String
    let snippets: String
    @Namespace private var pill

    var body: some View {
        HStack(spacing: 0) {
            segment(.words, words, "segment-words")
            segment(.snippets, snippets, "segment-snippets")
        }
        .padding(3)
        .overlay(Capsule().strokeBorder(Theme.outline, lineWidth: 1))
    }

    private func segment(_ value: WordsPart, _ label: String, _ identifier: String) -> some View {
        let on = value == part
        return Button {
            withAnimation(Theme.spring) { part = value }
        } label: {
            Text(label)
                .labelText(on ? 600 : 500)
                .foregroundStyle(on ? Theme.accent : Theme.secondary)
                .frame(maxWidth: .infinity, minHeight: 44)
                .background {
                    if on {
                        Capsule()
                            .fill(Theme.pill)
                            .matchedGeometryEffect(id: "pill", in: pill)
                    }
                }
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(identifier)
        .accessibilityAddTraits(on ? [.isSelected] : [])
    }
}

// MARK: - Adding a word

/// A word on its way into the Dictionary.
struct WordDraft: Identifiable {
    let id = UUID()
    /// A dictation whose words are offered as chips: "Add a word from this".
    var source = ""
    /// The word, already filled in.
    var word = ""
    /// The word is in the Dictionary already; this adds what it's heard as.
    var fixed = false
}

/// Adds a word to the Dictionary, with the form the speech model writes
/// instead when there is one. Opened from a dictation, its words are chips,
/// so a mishearing is one tap away.
struct AddWordSheet: View {
    @EnvironmentObject private var store: Store
    @Environment(\.dismiss) private var dismiss
    let draft: WordDraft

    @State private var word = ""
    @State private var heard = ""
    @State private var filled = false

    private var tokens: [String] {
        var seen: [String] = []
        for piece in draft.source.split(whereSeparator: \.isWhitespace) {
            let token = piece.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
            if token.count > 1, !seen.contains(token) { seen.append(token) }
            if seen.count == 16 { break }
        }
        return seen
    }

    private var picked: [String] {
        heard.split(separator: " ").map(String.init)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text(draft.fixed ? "How Legato hears \(draft.word)" : "Add to Dictionary")
                    .titleText()
                    .foregroundStyle(Theme.text)
                if !tokens.isEmpty {
                    Text("Tap the words Legato got wrong.")
                        .smallText()
                        .foregroundStyle(Theme.secondary)
                    FlowLayout(spacing: 8) {
                        ForEach(tokens, id: \.self) { token in
                            chip(token)
                        }
                    }
                }
                if !draft.fixed {
                    FormField(label: "Word, spelled right", text: $word)
                }
                FormField(
                    label: draft.fixed ? "Heard as" : "Heard as (optional)",
                    text: $heard,
                    help: "What the model writes instead, like “net bird”."
                )
                Button("Add") {
                    let text = word
                    let form = heard
                    store.run { try $0.addWordHeard(text: text, heard: form) }
                    dismiss()
                }
                .buttonStyle(FilledButton())
                .disabled(word.trimmingCharacters(in: .whitespaces).isEmpty
                    || (draft.fixed && heard.trimmingCharacters(in: .whitespaces).isEmpty))
                .padding(.top, 4)
                Button("Cancel") {
                    dismiss()
                }
                .buttonStyle(SoftButton(filled: false))
            }
            .padding(.horizontal, 24)
            .padding(.top, 32)
            .padding(.bottom, 24)
        }
        .scrollDismissesKeyboard(.interactively)
        .formSheet()
        .onAppear {
            if !filled {
                filled = true
                word = draft.word
            }
        }
    }

    private func chip(_ token: String) -> some View {
        let on = picked.contains { $0.caseInsensitiveCompare(token) == .orderedSame }
        return Button {
            if on {
                heard = picked.filter { $0.caseInsensitiveCompare(token) != .orderedSame }.joined(separator: " ")
            } else {
                heard = (picked + [token.lowercased()]).joined(separator: " ")
            }
        } label: {
            Text(token)
                .labelText()
                .foregroundStyle(on ? Theme.accent : Theme.secondary)
                .padding(.horizontal, 14)
                .frame(minHeight: 36)
                .background(on ? Theme.pill : Color.clear, in: Capsule())
                .overlay(Capsule().strokeBorder(on ? Color.clear : Theme.outline, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? [.isSelected] : [])
    }
}

// MARK: - A snippet

/// A snippet being made (no `editing`) or changed.
struct SnippetDraft: Identifiable {
    let id = UUID()
    var editing: Int64?
    var trigger = ""
    var text = ""
}

/// Adds a snippet or edits one: the phrase you say and what Legato types.
private struct SnippetSheet: View {
    @EnvironmentObject private var store: Store
    @Environment(\.dismiss) private var dismiss
    let draft: SnippetDraft

    @State private var trigger = ""
    @State private var text = ""
    @State private var filled = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text(draft.editing == nil ? "New snippet" : "Edit snippet")
                    .titleText()
                    .foregroundStyle(Theme.text)
                FormField(label: "When you say", text: $trigger, help: "A short phrase, like “my address”.")
                FormField(label: "Legato types", text: $text, lines: 3)
                Button("Save") {
                    let id = draft.editing
                    let phrase = trigger
                    let typed = text
                    store.run { try $0.saveSnippet(id: id, trigger: phrase, text: typed) }
                    dismiss()
                }
                .buttonStyle(FilledButton())
                .disabled(trigger.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .padding(.top, 4)
                Button("Cancel") {
                    dismiss()
                }
                .buttonStyle(SoftButton(filled: false))
            }
            .padding(.horizontal, 24)
            .padding(.top, 32)
            .padding(.bottom, 24)
        }
        .scrollDismissesKeyboard(.interactively)
        .formSheet()
        .onAppear {
            if !filled {
                filled = true
                trigger = draft.trigger
                text = draft.text
            }
        }
    }
}
