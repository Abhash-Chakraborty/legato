import LegatoCore
import SwiftUI

/// What Settings doesn't show: how the link to the PC stands in detail, and
/// the log of what the core did, newest first, to copy into a bug report.
struct AdvancedScreen: View {
    @EnvironmentObject private var store: Store

    private var reachedOver: String {
        if !store.state.online { return "Nothing right now" }
        switch store.state.via ?? "" {
        case "bluetooth": return "Bluetooth"
        case "netbird": return "A VPN address"
        default: return "The local network"
        }
    }

    var body: some View {
        let status = pcStatus(store.state, device: store.device, long: true)
        let newest = Array(store.lines.reversed())
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                Text("Advanced")
                    .pageTitle()
                    .foregroundStyle(Theme.text)
                    .padding(.top, 8)
                    .accessibilityAddTraits(.isHeader)

                GroupHeader("Connection")
                Card {
                    HStack(spacing: 10) {
                        StatusDot(color: status.mood.color)
                        Text(store.state.paired ? (store.state.pcName ?? "PC") : "Not paired")
                            .bodyText()
                            .foregroundStyle(Theme.text)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        if store.state.paired {
                            Button("Reconnect") {
                                store.run { try $0.connect() }
                            }
                            .labelText(600)
                            .foregroundStyle(Theme.accent)
                            .padding(.horizontal, 14)
                            .frame(minHeight: 36)
                            .overlay(Capsule().strokeBorder(Theme.outline, lineWidth: 1))
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.top, 14)
                    Text(store.state.paired ? status.text : "Pair from Settings › Your PC.")
                        .smallText()
                        .foregroundStyle(Theme.faint)
                        .padding(.leading, 34)
                        .padding(.trailing, 16)
                        .padding(.top, 4)
                        .padding(.bottom, 14)
                    Hairline()
                    Detail(label: "Reached over", value: reachedOver)
                    Hairline()
                    Detail(
                        label: "This \(store.device)'s key",
                        value: store.keySealed
                            ? "Sealed with a key in the Keychain"
                            : "In Legato's files; the Keychain didn't take it"
                    )
                    Hairline()
                    Detail(label: "Core", value: coreVersion())
                }

                GroupHeader("Log")
                if newest.isEmpty {
                    EmptyState(icon: "text.alignleft", text: "Connections and syncs show up here as they happen.")
                } else {
                    Card {
                        ForEach(Array(newest.enumerated()), id: \.offset) { index, line in
                            if index > 0 {
                                Hairline()
                            }
                            Text(line)
                                .font(.system(size: 12.5, design: .monospaced))
                                .foregroundStyle(Theme.secondary)
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 16)
                                .padding(.vertical, 10)
                        }
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 32)
        }
        .background(Theme.background.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    copyText(store.lines.joined(separator: "\n"))
                    store.show("Log copied")
                } label: {
                    Image(systemName: "doc.on.doc")
                }
                .disabled(store.lines.isEmpty)
                .accessibilityLabel("Copy the log")
            }
        }
    }
}

private struct Detail: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .smallText()
                .foregroundStyle(Theme.faint)
            Text(value)
                .bodyText()
                .foregroundStyle(Theme.text)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .accessibilityElement(children: .combine)
    }
}
