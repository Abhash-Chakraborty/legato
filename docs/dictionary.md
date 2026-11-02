# Dictionary and snippets

The Dictionary keeps names and terms spelled your way and learns from your fixes. Snippets paste saved text when you say a short trigger phrase.

## How the Dictionary works

The Dictionary is used before and after the speech model listens.

- **Before**: models that take a hint, like Whisper, get up to 40 of your words before each dictation. Pinned words go first, then the ones used most recently.
- **After**: the ways the model misheard a word, and close matches to your words, are corrected in what it wrote. This happens for every word, hinted or not.

Test a sentence at the top of the Dictionary page shows what would be corrected. Type how a sentence sounds, such as "restart doc ploy on the arm box", and the result marks each corrected word.

## Add a word

Choose Add word on the Dictionary page.

| Field          | What it does                                                                            |
| -------------- | --------------------------------------------------------------------------------------- |
| Word or name   | Spelled exactly the way it should be pasted, up to 60 characters                        |
| Also heard as  | Optional. How Legato mishears it, such as "doc ploy"; press Enter after each one        |
| What it is     | Word, Person, Project, Organisation or Term                                             |
| Case sensitive | Pasted with exactly this capitalisation, even when the model capitalises it differently |
| Pinned         | Always in the vocabulary hint; up to 20 words can be pinned                             |

Names, projects and terms are kept exactly as written. The AI never turns them into a similar ordinary word, and notes summaries spell them this way. A word you add is used from your next dictation.

## Fix a word on Home

When Legato gets a word wrong, hover the transcript in Home and choose Fix a word, or press F. Click the wrong word, or drag across a few, then type what it should be.

- **Fix and learn** fixes it and adds the word to your Dictionary. "Remember … as a way Legato mishears it" is ticked by default and saves what it heard as a heard-as form too. When the same mistake is in other dictations, tick "Also fix … other transcripts" to fix those as well.
- **Just fix here** changes this transcript only.

Words you fix this way show as "first fixed on Home" in the Dictionary.

## Add selection to Dictionary

Select a word in any app and press Win+Alt+W. Legato copies the selection, puts your clipboard back as it was, and adds the word spelled exactly as selected. The bar confirms it, or says the word is already there. A selection can be one word or a name up to four words long.

## Learn from my edits

When you correct a word right after Legato pastes it, Legato can learn the correction. Ten seconds after a paste, it reads the field once, on this PC, and compares it with what it pasted. The field's text stays in memory and is dropped after the comparison. Slack, Teams and VS Code often don't share their text, so it learns less there.

Learn from my edits is on by default. Find it under Learning and matching on the Dictionary page, with When it finds a word:

- **Ask first**, the default: the word waits under Needs review, and the bar says "Spotted …". Add it, Edit it first, or Don't learn. Add all takes every word at once. On the keyboard, Enter adds the selected word, E edits it and Delete turns it down.
- **Add automatically**: the word goes straight into the Dictionary, and the bar says so with Undo.

Words you turn down land in Never learn, and Learn from my edits never proposes them again. Edit the list there to let a word be learned again.

## Spelled differently each time

Sometimes the model writes the same term more than one way, such as "Coolify" and "cool if I". Legato looks through the last 30 days of dictations for terms written in more than one form at least three times, leaving out words already in your Dictionary. They show under Needs review as Spelled differently each time. Add one and every version comes out the same. Ignore stops the suggestion.

## Matching

Matching, under Learning and matching, sets how close a heard word must be to one of yours before Legato swaps it in.

| Setting | What it does                                                                     |
| ------- | -------------------------------------------------------------------------------- |
| Strict  | Only near-identical words are corrected. Fewest surprises in ordinary sentences. |
| Normal  | Close misses of your words are corrected. The default, and the recommended one.  |
| Loose   | Catches more misses, and may now and then correct a word that was right.         |

Vocabulary hint, in the same card, shows the list the speech model receives before each dictation, in order.

## AI correction pass

If you set up an AI provider, Settings › AI › Correction pass with your Dictionary gives your model each dictation with your words and their heard-as forms, and it fixes misses the rules didn't catch. It's off by default. Skip short dictations leaves out dictations under 8 words unless you change it. If the model hasn't answered after 6 seconds (Give up on a dictation after), the text is pasted without AI changes. See [Transforms, Ask and your AI provider](transforms.md).

## Browse, import and export

The tabs at the top split the list into All, Learned, Added by me and Needs review. Sort by Pinned first, Most fixed, Fixed most recently, Newest or A to Z. Each word shows where it came from and how often it fixed a transcript.

Import words… reads a CSV of words (one per line), a CSV of heard,word pairs, or JSON: Legato's own export or a plain list of words. Export as JSON… saves the whole Dictionary.

Your Dictionary syncs with [Legato for Android](android.md) when a phone is paired.

## Your name

Settings › General › Your name spells your name right in dictations and summaries, and Notes give tasks meant for you to You. It's kept in the Dictionary as a person's name. Add the ways Legato sometimes hears it under "Legato sometimes hears it as".

## Snippets

Say a trigger phrase and Legato pastes the saved text. Use snippets for text you type often, such as a sign-off, an address or a prompt. The Dictionary handles single-word spellings.

### Make a snippet

Choose New snippet on the Snippets page.

1. **Trigger**: a short phrase you wouldn't say by accident, such as "sign off". Up to 60 characters.
2. **Text**: what gets pasted, up to 4,000 characters. Line breaks are kept.
3. **Expand**: Anywhere in a sentence expands it wherever you say it, even mid-sentence. Only when said alone expands it only when the trigger is the whole dictation.
4. **Test**: type a sentence as you'd say it to see whether it expands.

The text is pasted exactly as written. Cleanup and Style don't change it, though they still tidy the rest of a sentence around it.

### Variables

Insert variable adds one of these to the text. They're filled in when the snippet is pasted.

| Variable        | Becomes                                            |
| --------------- | -------------------------------------------------- |
| `{{date}}`      | Today's date, such as 2026-11-02                   |
| `{{date:long}}` | Today's date written out, such as November 2, 2026 |
| `{{time}}`      | The time, such as 14:05                            |
| `{{clipboard}}` | What's on the clipboard                            |
| `{{selection}}` | The text selected in the app                       |

### How triggers are matched

Snippets are matched after Dictionary fixes and before cleanup. A trigger matches when the spoken words are close to it, so punctuation, capitals and a small mishearing don't stop it. Longer triggers are tried first.

Since the Dictionary runs first, a trigger that contains one of its heard-as forms may not match as you expect. The editor warns about that: "The Dictionary changes … to … first; use that spelling in the trigger."

### Manage snippets

Sort by Most used, Recently used, Newest or A to Z. Select several to export or delete them together. Import snippets reads JSON or CSV, and Export snippets saves JSON. Each snippet shows how often it expanded and where it was last used.

Snippets sync with [Legato for Android](android.md) too.

## See also

- [Dictation](dictation.md) for where the Dictionary and snippets sit in each dictation's steps.
- [Style](style.md) for cleanup and the tone for each kind of app.
- [Home, Scratchpad and Insights](scratchpad.md) for Home and the accuracy numbers in Insights.
