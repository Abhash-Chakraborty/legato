import LegatoCore
import SwiftUI
import UIKit

/// Your PC, the connection, and what this app is.
struct SettingsScreen: View {
    @EnvironmentObject private var store: Store
    let openPair: () -> Void
    let openAdvanced: () -> Void

    @State private var menu: MenuModel?
    @State private var confirmUnpair = false

    private var pc: String {
        store.state.pcName ?? "your PC"
    }

    private var version: String {
        let app = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        return dotted([app, "core \(coreVersion())"])
    }

    var body: some View {
        Page("Settings") {
            GroupHeader("Your PC")
            if let failure = store.failure {
                Card {
                    SettingRow("Legato couldn't open its files", detail: failure)
                }
            } else if store.state.paired {
                PcCard(
                    name: store.state.pcName ?? "PC",
                    status: pcStatus(store.state, device: store.device, long: true),
                    online: store.state.online,
                    sync: { store.syncNow() },
                    more: { menu = choices() }
                )
            } else {
                Card {
                    if let ended = store.ended {
                        HStack(alignment: .top, spacing: 10) {
                            StatusDot(color: LinkMood.ended.color)
                                .padding(.top, 6)
                            Text("The pairing ended: \(plainWords(ended, pc: nil))")
                                .smallText()
                                .foregroundStyle(Theme.secondary)
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                        Hairline()
                    }
                    SettingRow(
                        "Pair with your PC",
                        detail: "With the code or the link in Legato on Windows › Settings › Devices.",
                        action: openPair
                    ) {
                        Chevron()
                    }
                    .accessibilityIdentifier("settings-pair")
                }
            }

            GroupHeader("Connection")
            Card {
                SettingRow(
                    "Over your network",
                    detail: "Legato syncs while it's open and this \(store.device) is on your PC's Wi-Fi or VPN. Changes made away from it wait here and go over next time."
                )
                Hairline()
                SettingRow(
                    "Local network access",
                    detail: "iOS asks for it the first time Legato looks for your PC. If you said no, turn it on for Legato in the Settings app.",
                    action: nil
                ) {
                    Button("Open") {
                        if let url = URL(string: UIApplication.openSettingsURLString) {
                            UIApplication.shared.open(url)
                        }
                    }
                    .labelText(600)
                    .foregroundStyle(Theme.accent)
                    .frame(minWidth: 44, minHeight: 44)
                    .accessibilityLabel("Open the Settings app")
                }
            }

            GroupHeader("About")
            Card {
                SettingRow("Legato for \(store.device)", detail: version)
                Hairline()
                SettingRow("Advanced", detail: "Connection details and the log.", action: openAdvanced) {
                    Chevron()
                }
                .accessibilityIdentifier("settings-advanced")
            }
        }
        .menuSheet($menu)
        .alert("Unpair from \(pc)?", isPresented: $confirmUnpair) {
            Button("Unpair", role: .destructive) {
                store.run { try $0.unpair() }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("What already synced stays on this \(store.device); nothing new syncs until you pair again.")
        }
    }

    private func choices() -> MenuModel {
        MenuModel(
            title: pc,
            subtitle: pcStatus(store.state, device: store.device, long: true).text,
            actions: [
                MenuAction(label: "Reconnect", icon: "arrow.clockwise") {
                    store.run { try $0.connect() }
                },
                MenuAction(label: "Connection details", icon: "slider.horizontal.3") {
                    openAdvanced()
                },
                MenuAction(label: "Unpair", icon: "minus.circle", danger: true) {
                    confirmUnpair = true
                },
            ]
        )
    }
}

/// The paired PC: its mark, name and status, with Sync now and More.
private struct PcCard: View {
    let name: String
    let status: PcStatus
    let online: Bool
    let sync: () -> Void
    let more: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 14) {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 20, weight: .medium))
                    .foregroundStyle(Color.white)
                    .frame(width: 48, height: 48)
                    .background(Theme.fill, in: Circle())
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 3) {
                    Text(name)
                        .bodyText(600)
                        .foregroundStyle(Theme.text)
                    HStack(alignment: .top, spacing: 6) {
                        StatusDot(color: status.mood.color)
                            .padding(.top, 5)
                        Text(status.text)
                            .smallText()
                            .foregroundStyle(Theme.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack(spacing: 8) {
                Button(action: sync) {
                    Label(online ? "Sync now" : "Try again", systemImage: "arrow.triangle.2.circlepath")
                }
                .buttonStyle(SoftButton())
                Button(action: more) {
                    Label("More", systemImage: "ellipsis")
                }
                .buttonStyle(SoftButton(filled: false))
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 18)
        .padding(.bottom, 16)
        .background(Theme.pill.opacity(0.45), in: RoundedRectangle(cornerRadius: 20, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("pc-card")
    }
}
