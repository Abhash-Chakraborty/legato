import XCTest

/// Walks the app in the simulator against the real core, and keeps a
/// screenshot of each screen in light and in dark.
///
/// `-seedDemo` starts the app on a new folder filled with made-up content
/// through the core; `-freshData` on an empty one. Screenshots are attached
/// to the test results and, when LEGATO_SHOTS_DIR is set, written there too.
final class LegatoUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        // One missing element shouldn't hide what the rest of a walk shows.
        continueAfterFailure = true
    }

    // MARK: Helpers

    private func launch(_ arguments: [String], look: String = "light") {
        app = XCUIApplication()
        app.launchArguments = arguments + ["-appearance", look]
        app.launch()
    }

    private func shot(_ name: String) {
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let dir = ProcessInfo.processInfo.environment["LEGATO_SHOTS_DIR"], !dir.isEmpty else { return }
        let folder = URL(fileURLWithPath: dir, isDirectory: true)
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? screenshot.pngRepresentation.write(to: folder.appendingPathComponent(name + ".png"))
    }

    /// The element, once it's on screen; a failure at the caller's line when it isn't.
    @discardableResult
    private func see(
        _ element: XCUIElement, _ what: String, timeout: TimeInterval = 10,
        file: StaticString = #filePath, line: UInt = #line
    ) -> Bool {
        let there = element.waitForExistence(timeout: timeout)
        XCTAssertTrue(there, "\(what) should be on screen", file: file, line: line)
        return there
    }

    /// Taps the element if it's there, and fails at the caller's line if not.
    private func tap(
        _ element: XCUIElement, _ what: String, timeout: TimeInterval = 10,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        if see(element, what, timeout: timeout, file: file, line: line) {
            element.tap()
        }
    }

    private func gone(
        _ element: XCUIElement, _ what: String, timeout: TimeInterval = 10,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        let end = Date().addingTimeInterval(timeout)
        while element.exists, Date() < end {
            Thread.sleep(forTimeInterval: 0.2)
        }
        XCTAssertFalse(element.exists, "\(what) should have gone", file: file, line: line)
    }

    /// Anything whose label has these words: a line of text, or a row that reads them out.
    private func text(containing piece: String) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", piece)).firstMatch
    }

    private func button(containing piece: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label CONTAINS %@", piece)).firstMatch
    }

    /// A field by its label, whether it's one line or grows to several.
    private func field(_ label: String) -> XCUIElement {
        let several = app.textViews[label]
        return several.exists ? several : app.textFields[label]
    }

    /// Any element with this accessibility identifier, whatever kind it is.
    private func element(_ identifier: String) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: identifier).firstMatch
    }

    private func tab(_ name: String) {
        tap(app.buttons["tab-\(name)"], "the \(name) tab")
    }

    /// Back from a screen opened over the tabs.
    private func back() {
        let bar = app.navigationBars.firstMatch
        let named = bar.buttons["Back"]
        if named.exists {
            named.tap()
        } else {
            tap(bar.buttons.firstMatch, "the back button")
        }
    }

    /// Types into a field, tapping it first for the cursor unless it has it.
    private func enter(
        _ words: String, into field: XCUIElement, _ what: String, tapFirst: Bool = true,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        if see(field, what, file: file, line: line) {
            if tapFirst { field.tap() }
            field.typeText(words)
        }
    }

    // MARK: The walk

    func testWalkTheTabsInLight() {
        walk(look: "light")
    }

    func testWalkTheTabsInDark() {
        walk(look: "dark")
    }

    /// Every tab and every screen behind them, on seeded content.
    private func walk(look: String) {
        launch(["-seedDemo"], look: look)

        // Home: the dictations, and where to start before pairing.
        see(app.staticTexts["Home"], "Home's title", timeout: 30)
        see(app.buttons["pair-card"], "the Pair with your PC card")
        see(text(containing: "Can we move lunch to one?"), "a seeded dictation")
        see(app.staticTexts["Today"], "the Today header")
        shot("\(look)-01-home")

        enter("lunch\n", into: app.textFields["Search your dictations"], "Home's search")
        see(text(containing: "Can we move lunch to one?"), "the dictation that matches")
        gone(text(containing: "water the plants"), "a dictation that doesn't match")
        shot("\(look)-02-home-search")
        tap(app.buttons["Clear search"], "Clear search")
        see(text(containing: "water the plants"), "the whole list again")

        tap(app.buttons["More for this dictation"].firstMatch, "a dictation's menu button")
        see(app.buttons["Add a word from this"], "the dictation's menu")
        shot("\(look)-03-home-menu")
        tap(app.buttons["Add a word from this"], "Add a word from this")
        see(app.staticTexts["Tap the words Legato got wrong."], "the Add to Dictionary sheet")
        shot("\(look)-04-add-word-from-dictation")
        tap(app.buttons["Cancel"], "Cancel")

        // Words: the Dictionary, then snippets.
        tab("words")
        see(app.staticTexts["Dictionary"], "the Dictionary title")
        see(app.buttons["word-Okonkwo"], "a seeded word")
        see(text(containing: "Heard as “oh conk woe”"), "what the word is heard as")
        shot("\(look)-05-words")
        tap(app.buttons["word-Okonkwo"], "a word")
        see(app.buttons["Remove from Dictionary"], "the word's menu")
        shot("\(look)-06-word-menu")
        tap(app.buttons["Copy"], "Copy")
        see(app.staticTexts["Copied"], "the Copied toast")

        tap(app.buttons["segment-snippets"], "the Snippets segment")
        see(app.staticTexts["Snippets"], "the Snippets title")
        see(app.buttons["snippet-my address"], "a seeded snippet")
        shot("\(look)-07-snippets")
        tap(button(containing: "Add snippet"), "Add snippet")
        see(app.staticTexts["New snippet"], "the New snippet sheet")
        shot("\(look)-08-new-snippet")
        tap(app.buttons["Cancel"], "Cancel")

        // Scratchpad: the list and a note.
        tab("scratchpad")
        see(app.staticTexts["Scratchpad"], "the Scratchpad title")
        see(app.buttons["scratch-Packing list"], "a seeded note")
        shot("\(look)-09-scratchpad")
        tap(app.buttons["scratch-Packing list"], "a note")
        see(app.textFields["Title"], "the note's title field")
        shot("\(look)-10-scratchpad-note")
        back()

        // Notes: the list and a memo's transcript.
        tab("notes")
        see(app.staticTexts["Notes"], "the Notes title")
        see(app.buttons["note-Thoughts after the call"], "a seeded memo")
        shot("\(look)-11-notes")
        tap(app.buttons["note-Thoughts after the call"], "a memo")
        see(app.staticTexts["Transcript"], "the transcript")
        see(text(containing: "Ines wants the colour samples"), "a line of the transcript")
        shot("\(look)-12-note")
        back()

        // Settings, Advanced and pairing.
        tab("settings")
        see(app.staticTexts["Settings"], "the Settings title")
        see(app.staticTexts["Your PC"], "the Your PC group")
        see(app.staticTexts["Connection"], "the Connection group")
        see(app.staticTexts["About"], "the About group")
        shot("\(look)-13-settings")
        tap(app.buttons["settings-advanced"], "Advanced")
        see(app.staticTexts["Advanced"], "the Advanced title")
        see(app.staticTexts["Not paired"], "the link's state")
        see(app.staticTexts["Log"], "the log")
        shot("\(look)-14-advanced")
        back()

        tap(app.buttons["settings-pair"], "Pair with your PC")
        see(app.buttons["pair-button"], "the Pair button")
        XCTAssertFalse(app.buttons["pair-button"].isEnabled, "Pair waits for a link")
        shot("\(look)-15-pair")
    }

    // MARK: Edits against the core

    /// Delete lives in a row's menu and always offers Undo.
    func testRemovingAWordOffersUndo() {
        launch(["-seedDemo"])
        tab("words")
        let figma = app.buttons["word-Figma"]
        tap(figma, "the word")
        tap(app.buttons["Remove from Dictionary"], "Remove from Dictionary")
        see(app.staticTexts["Removed Figma"], "the toast")
        gone(figma, "the removed word", timeout: 3)
        shot("light-16-removed-with-undo")
        tap(app.buttons["Undo"], "Undo")
        see(figma, "the word, back")
        gone(app.staticTexts["Removed Figma"], "the toast")
    }

    /// A word with what it's heard as, a snippet and a Scratchpad note,
    /// each typed in and read back from the core.
    func testAddingAWordASnippetAndANote() {
        launch(["-freshData"])
        see(app.staticTexts["Your dictations show up here."], "Home's empty state", timeout: 30)
        shot("light-17-home-empty")

        tab("words")
        see(text(containing: "Names and terms you add here"), "the Dictionary's empty state")
        tap(button(containing: "Add word"), "Add word")
        enter("Solveig", into: app.textFields["Word, spelled right"], "the word field")
        enter("sol vague", into: app.textFields["Heard as (optional)"], "the heard-as field")
        tap(app.buttons["Add"], "Add")
        see(app.buttons["word-Solveig"], "the new word")
        see(text(containing: "Heard as “sol vague”"), "what it's heard as")
        see(app.buttons["segment-words"], "the Words segment")
        XCTAssertEqual(app.buttons["segment-words"].label, "Words · 1")

        tap(app.buttons["segment-snippets"], "the Snippets segment")
        see(text(containing: "Say a trigger phrase"), "the snippets' empty state")
        tap(button(containing: "Add snippet"), "Add snippet")
        enter("my address", into: app.textFields["When you say"], "the trigger field")
        enter("14 Linden Road", into: field("Legato types"), "the text field")
        tap(app.buttons["Save"], "Save")
        see(app.buttons["snippet-my address"], "the new snippet")
        see(text(containing: "14 Linden Road"), "the snippet's text")

        tab("scratchpad")
        see(text(containing: "Notes you type here sync"), "the Scratchpad's empty state")
        tap(button(containing: "New note"), "New note")
        enter("Shopping", into: app.textFields["Title"], "the title field")
        enter("Tomatoes and basil", into: app.textViews["Note"], "the note's text")
        shot("light-18-new-note")
        back()
        see(app.buttons["scratch-Shopping"], "the saved note")
        see(text(containing: "Tomatoes and basil"), "the note's preview")
    }

    /// A pairing link is shown before it pairs, and a bad one says why.
    func testAPairingLinkIsShownBeforeItPairs() {
        launch(["-freshData"])
        tab("settings")
        tap(app.buttons["settings-pair"], "Pair with your PC")
        let link = app.textFields["Pairing link"]

        enter("legato://pair?v=1&id=6f1c", into: link, "the link field")
        tap(app.buttons["pair-button"], "Pair")
        see(element("pair-error"), "why the link didn't work")
        shot("light-19-pair-bad-link")

        // The cursor is still at the end of the field.
        enter(
            "&name=Desk%20PC&fp=q-_x&addr=192.168.1.20:47613&secret=s3cr3t", into: link, "the link field",
            tapFirst: false
        )
        tap(app.buttons["pair-button"], "Pair")
        let question = app.alerts.firstMatch
        see(question, "the question before pairing")
        see(question.staticTexts["Pair with Desk PC?"], "the PC's name")
        see(text(containing: "192.168.1.20:47613"), "the PC's address")
        shot("light-20-pair-question")
        tap(question.buttons["Cancel"], "Cancel")

        back()
        see(app.buttons["settings-pair"], "Settings, still unpaired")
    }

    // MARK: A real pairing

    /// Opens a pairing link from a stand-in PC on this Mac, as the Camera
    /// would from the PC's code, pairs, and reads what comes over.
    func testPairingWithAStandInPC() throws {
        guard let link = ProcessInfo.processInfo.environment["LEGATO_PAIR_LINK"],
              let url = URL(string: link)
        else {
            throw XCTSkip("No stand-in PC is running (LEGATO_PAIR_LINK isn't set).")
        }
        launch(["-freshData"])
        see(app.staticTexts["Your dictations show up here."], "Home's empty state", timeout: 30)
        app.open(url)

        let question = app.alerts.firstMatch
        see(question, "the question before pairing", timeout: 20)
        see(question.staticTexts["Pair with Desk PC?"], "the PC's name")
        shot("light-21-pair-opened-link")
        tap(question.buttons["Pair"], "Pair")
        see(app.staticTexts["Paired with Desk PC"], "the paired screen", timeout: 20)
        shot("light-22-paired")
        tap(app.buttons["paired-done"], "Done")

        // Home: the PC's dictations, and the chip that says how it stands.
        see(text(containing: "Running ten minutes late"), "a dictation from the PC", timeout: 20)
        see(text(containing: "On Desk PC"), "where it was made")
        see(app.buttons["sync-chip"], "the PC's chip")
        XCTAssertTrue(app.buttons["sync-chip"].label.hasPrefix("Desk PC · "), app.buttons["sync-chip"].label)
        shot("light-23-home-synced")

        tab("words")
        see(app.buttons["word-Okonkwo"], "a word from the PC")
        shot("light-24-words-synced")

        // The PC's meeting: summary, action items, transcript.
        tab("notes")
        tap(app.buttons["note-Garden club planning"], "the PC's meeting")
        see(app.staticTexts["Summary"], "the summary")
        see(app.staticTexts["Action items"], "the action items")
        let action = app.buttons["action-Draft the poster"]
        see(action, "an action item")
        XCTAssertEqual(action.value as? String, "Not done")
        shot("light-25-meeting")
        action.tap()
        let ticked = NSPredicate(format: "value == %@", "Done")
        expectation(for: ticked, evaluatedWith: action)
        waitForExpectations(timeout: 10)
        shot("light-26-meeting-action-done")
        back()
        see(text(containing: "2 open actions"), "one action fewer in the list")

        // Settings: connected, with Sync now; Advanced has the log.
        tab("settings")
        see(element("pc-card"), "the PC's card")
        see(text(containing: "Connected on your network"), "how it's connected")
        tap(button(containing: "Sync now"), "Sync now")
        shot("light-27-settings-paired")
        tap(app.buttons["settings-advanced"], "Advanced")
        see(text(containing: "Connected to Desk PC"), "the connection in the log")
        see(text(containing: "Sealed with a key in the Keychain"), "the key's seal", timeout: 3)
        shot("light-28-advanced-paired")
        back()

        // The same screens in dark would repeat this; one of them is enough.
        tap(button(containing: "More"), "More")
        see(app.buttons["Unpair"], "the PC's menu")
        shot("light-29-pc-menu")
        tap(app.buttons["Unpair"], "Unpair")
        let confirm = app.alerts.firstMatch
        see(confirm, "the question before unpairing")
        tap(confirm.buttons["Unpair"], "Unpair")
        see(app.buttons["settings-pair"], "Settings, unpaired again", timeout: 15)
        tab("home")
        see(text(containing: "Running ten minutes late"), "what synced, kept after unpairing")
    }
}
