# USFM Language Server Implementation

The VS Code extension. It contributes the USFM language, the syntax grammar,
the preview and the dictionary editor, and it starts the language server —
`apps/usfm_language_server`, the `usfm-language-server` binary (ticket 30).

## Settings

| setting | what it does |
|---|---|
| `usfm.enable` | start the server at all |
| `usfm.path.server` | a server binary to use instead of the bundled one |
| `usfm.stylesheet` | the project's `.sty`, absolute or relative to the workspace folder. It **extends** the default stylesheet; with no setting the server uses a `custom.sty` beside the open file, if there is one |
| `usfm.requireConfig` | start only when the workspace has a `usfm.config.json` |

`usfm.stylesheet` is sent to the server as `initializationOptions.stylesheet`
and is read once per path: after editing a `.sty`, run **USFM: Restart USFM
Server**.

## Building

```bash
npm install
npm run server:build:debug     # cargo build -p usfm_language_server
npm run compile                # bundle client/extension.ts into out/main.js
npm run notices                # ThirdPartyNotices.txt, which the .vsix carries
npm run build                  # release server + bundle + .vsix
```

## The manual check: diagnostics, formatting and hover in the editor

CI runs the server's own tests (`cargo test -p usfm_language_server`, which
includes a JSON-RPC conversation with the built binary over stdio), but nothing
in CI runs VS Code. This is the check by hand, and it is the one ticket 30's
"done when" asks for:

1. `cargo build -p usfm_language_server` at the repository root.
2. Open the repository in VS Code and press F5 ("Launch Client" in
   `.vscode/launch.json`). It starts an Extension Development Host with
   `SERVER_PATH_DEV` pointing at `target/debug/usfm-language-server`.
   (Without F5: `npm run server:build:release && npm run reinstall` installs
   the packaged extension into your own VS Code instead.)
3. In the new window, create a file called `check.usfm` and paste:

   ```usfm
   \id GEN
   \c 1
   \p
   \v 1 text \qqq more
   ```

4. `\qqq` is underlined in red. Hovering it says
   `unknown marker \qqq`, and the Problems panel shows one entry, source
   `usfm`, code `unknown-marker`, at line 4 column 11.
5. Delete `\qqq` and save. The problem disappears as you type — the server
   parses on every change and publishes an empty list when there is nothing
   left to report.
6. For the stylesheet rule: put a `custom.sty` next to `check.usfm` holding

   ```
   \Marker qqq
   \StyleType Character
   ```

   and restart the server (**USFM: Restart USFM Server**; a `.sty` is read
   once per path). Change the line to `\v 1 text \qqq more\qqq* end` —
   `\qqq` is now a known character marker, and with its closing marker the
   file reports nothing at all. Point
   `usfm.stylesheet` at a file that does not exist and the server says so in a
   warning notification once, then carries on with the default stylesheet.

### Formatting and hover (ticket 31)

7. Hover the `\p` on line 3. A tooltip says
   `\p — p - Paragraph - Normal - First Line Indent`, the marker's
   description under it, and the `\OccursUnder` list on one line — all of it
   from the stylesheet the file was parsed with, so a marker from your
   `custom.sty` describes itself the same way. Hover the `\v` (or any word of
   the verse text) and the tooltip is the reference, `GEN 1:1`.
8. Put the verse on a line of its own — `\p` on one line and
   `\v 1 text more` under it — and run **Format Document**
   (`shift+alt+F`). The file comes back in the writer's canonical shape: one
   line per paragraph, `\v 1` on the `\p` line, `\ca`/`\cp` on lines of
   their own. It is exactly what
   `usfm format` writes, so a file formatted here and one formatted in a
   script are the same bytes.
9. Turn on formatting on save for USFM:

   ```json
   "[usfm]": { "editor.formatOnSave": true }
   ```

   Saving now formats. The server advertises `documentFormattingProvider`, so
   nothing else has to be configured for the command or the save to find it.
10. Put `\qqq` back and save. **Nothing is written**, and a warning
    notification says the file has an error and formatting it would write the
    repaired text over yours. That is the same rule as `usfm format --write`
    without `--force`: a parse that reported an error was repaired, and the
    repaired tree is not the author's text.

### The outline, completion and quick fixes (ticket 32)

Start from a file with two chapters, which is what the outline is for:

```usfm
\id GEN
\c 1
\p
\v 1 In the beginning
\v 2 and the earth
\c 2
\p
\v 1 thus the heavens
```

11. Open the **Outline** view (Explorer sidebar, or `Ctrl+Shift+O` for "go to
    symbol in file"). It shows `GEN` with `Chapter 1` and `Chapter 2` under
    it, and `1`, `2` under the first. Clicking a verse jumps to its `\v` and
    selects the marker; the breadcrumb bar at the top of the editor shows
    `GEN > Chapter 1 > 1` as the cursor moves. A `\v 1-2` shows as `1-2`, a
    number written twice shows twice — the outline is of the file, not of the
    versification. An `\esb` sidebar appears as `Sidebar` where it stands,
    and a `\periph` as `Periph <title>`.
12. Put the cursor at the end of the `\v 1` line and type a `\`. The
    completion list opens by itself (`\` is the trigger character); each item
    shows the marker with its `\Name` beside it and its `\Description` in the
    details pane (`Ctrl+Space` toggles that pane open). Type `nd` and press
    Enter: the line gets `\nd \nd*` with the cursor between the two, because
    a character style is inserted as a snippet with its closing marker. Note
    that the `\` you typed is not doubled.
13. The list is filtered by where the cursor is. In the paragraph, `\fq` is
    **not** offered — it occurs only inside a note, and writing it there is
    `marker-not-allowed-here`. Write a footnote (`\f + \ft note\f*`), put the
    cursor inside it before `\f*`, type `\`, and `\fq` is offered there. A
    marker from your `custom.sty` is offered like any other: the list is the
    sheet the file was parsed with.
14. Quick fixes. With `\qqq` in the file again, put the cursor on it and
    press `Ctrl+.` (or click the lightbulb). One action is offered, **Delete
    `\qqq`**; applying it removes the marker and the space after it, and the
    problem goes. The other four:
    - `\em text` with no `\em*` (a warning) offers **Close `\em` with
      `\em*`**, which writes the closer at the end of the styled text;
    - `\f \ft note\f*` (no caller) offers **Add the `+` caller to `\f`**;
    - `\w word|lemma=grace\w*` offers **Put `grace` in quotes**;
    - `\ts-s |\*` offers **Delete the empty attribute list**.

    Every one of them is the repair the parser already made, written back
    into the file, so the diagnostic disappears and no new one appears.
    Diagnostics without an obvious edit (`missing-id`, `verse-out-of-order`,
    `marker-not-allowed-here`) offer no action: those are edits for a person.
