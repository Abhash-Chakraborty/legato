import LegatoCore
import SwiftUI

private let linkPrefix = "legato://pair?"

/// Pairing with the PC: open the link from its pairing code with the
/// Camera, or paste the one from Copy link. Every link, opened or pasted,
/// is shown first (the PC's name, its addresses, the PC it replaces) and
/// pairs only when you say so, since a link from someone else would sync
/// this phone with their computer.
struct PairScreen: View {
    @EnvironmentObject private var store: Store
    @Environment(\.dismiss) private var dismiss
    /// The link this screen was opened with, when there was one.
    let link: String?

    /// A link and what it would pair with, while you decide.
    private struct Question {
        let link: String
        let preview: PairPreview
    }

    @State private var pasted = ""
    @State private var busy = false
    @State private var error: String?
    @State private var question: Question?
    @State private var asking = false
    @State private var paired: String?
    @State private var opened = false

    init(link: String?) {
        self.link = link
    }

    var body: some View {
        Group {
            if let paired {
                PairedView(pc: paired, device: store.device) { dismiss() }
            } else {
                form
            }
        }
        .background(Theme.background.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .alert("Pair with \(question?.preview.pcName ?? "this PC")?", isPresented: $asking, presenting: question) { question in
            Button("Pair") {
                pair(question.link)
            }
            Button("Cancel", role: .cancel) {}
        } message: { question in
            Text(message(question.preview))
        }
        .onAppear {
            guard !opened else { return }
            opened = true
            if let link, !link.isEmpty {
                // Once the screen has slid in, so the question isn't lost.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) {
                    ask(link)
                }
            }
        }
    }

    private var form: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Pair with your PC")
                    .pageTitle()
                    .foregroundStyle(Theme.text)
                    .padding(.top, 8)
                    .accessibilityAddTraits(.isHeader)
                Text("On your PC, open Legato › Settings › Devices › Pair a phone. Point this \(store.device)'s Camera at the code and tap the Legato link it shows, or choose Copy link there and paste it here.")
                    .bodyText()
                    .foregroundStyle(Theme.text)
                Text("This \(store.device) syncs over Wi-Fi when it's on the PC's network.")
                    .smallText()
                    .foregroundStyle(Theme.secondary)

                VStack(alignment: .leading, spacing: 6) {
                    Text("Pairing link")
                        .smallText(500)
                        .foregroundStyle(Theme.secondary)
                        .accessibilityHidden(true)
                    TextField("", text: $pasted, prompt: Text(linkPrefix + "…").foregroundStyle(Theme.faint))
                        .bodyText()
                        .foregroundStyle(Theme.text)
                        .keyboardType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .submitLabel(.done)
                        .padding(.horizontal, 14)
                        .frame(minHeight: 48)
                        .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .strokeBorder(Theme.outline, lineWidth: 1)
                        )
                        .accessibilityLabel("Pairing link")
                        .onChange(of: pasted) { error = nil }
                }
                .padding(.top, 8)

                if let error {
                    Text(error)
                        .smallText()
                        .foregroundStyle(Theme.bad)
                        .accessibilityIdentifier("pair-error")
                }

                Button(busy ? "Pairing…" : "Pair") {
                    ask(pasted)
                }
                .buttonStyle(FilledButton())
                .disabled(busy || !pasted.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix(linkPrefix))
                .accessibilityIdentifier("pair-button")
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 24)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private func message(_ preview: PairPreview) -> String {
        var lines = [preview.addresses.joined(separator: " · ")]
        if let replaces = preview.replaces {
            lines.append("This replaces \(replaces), which stops syncing with this \(store.device).")
        }
        lines.append("Only pair with a code or link your own PC shows in Settings › Devices.")
        return lines.joined(separator: "\n\n")
    }

    /// Reads the link without pairing, and asks.
    private func ask(_ link: String) {
        guard !busy, !asking else { return }
        let link = link.trimmingCharacters(in: .whitespacesAndNewlines)
        store.ask({ try $0.inspect(link: link) }, then: { answer in
            switch answer {
            case let .success(preview):
                error = nil
                question = Question(link: link, preview: preview)
                asking = true
            case let .failure(problem):
                error = sentence(coreWords(problem))
            }
        })
    }

    private func pair(_ link: String) {
        busy = true
        error = nil
        store.ask(reload: true, { try $0.pair(link: link) }, then: { answer in
            busy = false
            switch answer {
            case .success:
                paired = question?.preview.pcName ?? store.state.pcName ?? "your PC"
            case let .failure(problem):
                error = sentence(coreWords(problem))
            }
        })
    }

    /// The core's own words, as a sentence.
    private func sentence(_ words: String) -> String {
        guard let first = words.first else { return "That link didn't work; copy it again." }
        let text = first.uppercased() + words.dropFirst()
        return text.hasSuffix(".") ? text : text + "."
    }
}

/// Paired: a tick, what happens next, and Done.
private struct PairedView: View {
    let pc: String
    let device: String
    let done: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Image(systemName: "checkmark")
                .font(.system(size: 26, weight: .semibold))
                .foregroundStyle(Color.white)
                .frame(width: 64, height: 64)
                .background(Theme.fill, in: Circle())
                .padding(.top, 48)
                .accessibilityHidden(true)
            Text("Paired with \(pc)")
                .pageTitle()
                .foregroundStyle(Theme.text)
                .padding(.top, 20)
                .accessibilityAddTraits(.isHeader)
            Text("Your Dictionary, snippets, history, notes and Scratchpad are coming over now. From here on, changes go both ways whenever the two can reach each other.")
                .bodyText()
                .foregroundStyle(Theme.secondary)
                .padding(.top, 12)
            Spacer(minLength: 24)
            Button("Done", action: done)
                .buttonStyle(FilledButton())
                .accessibilityIdentifier("paired-done")
        }
        .padding(.horizontal, 24)
        .padding(.bottom, 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .navigationBarBackButtonHidden(true)
    }
}
