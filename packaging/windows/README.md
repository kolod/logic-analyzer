# Windows packaging notes

What the installer (a later feature) must register. Nothing here is installed by the application
itself.

- **Executable icon**: `crates/app/assets/icon/app.ico` (16, 32, 48 and 256 pixels). The build puts it
  into `logic-analyzer.exe` when the resource compiler is available (`crates/app/build.rs`).
- **File association**: the extension `.lasession` (a session file) with the ProgID
  `LogicAnalyzer.Session`, the description "Logic analyzer session", the icon
  `crates/app/assets/icon/session.ico` and the open command `"<install dir>\logic-analyzer.exe" "%1"`.
- The artwork is original to the project (GPL-3.0); `tools/make_icons.py` regenerates every raster
  and `.ico` from the shapes of the two SVG files next to them.
