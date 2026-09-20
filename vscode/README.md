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
npm run build                  # release server + bundle + .vsix
```

## The manual check: diagnostics in the editor

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
