# Changelog

[Русский](CHANGELOG.md) · **English**

## [1.2.0]

### Added
- **Language-pair workspaces:** multiple translations (`source → target`) in a single project, each with its own database and progress, with quick switching (the "Translations" widget).
- **Translation Memory (TM):** auto-accumulation of translated pairs, auto-fill of exact matches flagged "to review", a memory editor.
- **Manual strings:** add, edit and delete translatable strings the extractor missed (text-keyed delivery). A dedicated "Manual strings" pseudo-file and adding directly into the open file.
- **Delivery channel override** per string (dialogue / interface / both) — for the "translated, but not shown in game" case.
- **Source viewer:** `.rpyc` decompilation (unrpyc) with syntax highlighting and a minimap; extracted strings are highlighted, potentially missed ones are offered for adding.
- **Expert delivery hooks:** custom Python/Ren'Py code woven into delivery, for non-standard statements (scope — global or per-project; syntax check before saving).
- **Cloud LLM API:** translation via an OpenAI-compatible endpoint (OpenAI, OpenRouter, DeepSeek, Groq, LM Studio, etc.) in addition to local Ollama and the clipboard.
- **Batch translation in chunks** with control over model response alignment and a cancel option.
- **Placeholder protection** for `[var]`: blocks saving and delivery of translations with foreign variables.
- **Build for distribution:** "Full game" export (a copy with the embedded translation) and "Translation only" (an overlay mod); copy progress, free-space check, folder overwrite.
- **Remove mod from game:** roll back the injected translation to the original (the translation database is kept).
- **"About" screen:** version, third-party components and licenses (with opening the licenses folder); license texts ship with the app.

### Changed
- **Translation delivery no longer switches the game language** — language-independent runtime text substitution, with no conflict with the game's own localization selector.
- **Extraction:** support for `_()` in `layeredimage`/screen, `renpy.input` prompts, UI strings of SL1 screens on legacy engines; added delivery channels (`translate_string`/`store._`, `renpy.ui.text` wrapper) for UI and pre-framework engines.
- **Editor performance** on large files: per-file DB index, lazy auto-grow of input fields, glossary highlight caching.

### Compatibility
- An offline WebView2 runtime is bundled into the installer.
- Write-permission check (warning for games in `Program Files`/Steam).
- Export resilience to a running game (skips locked files).

### Removed
- The interactive file graph (Vue Flow) — replaced by a dashboard with a file list, progress and the build pipeline.

## [1.1.0]
- Initial release: text extraction from `.rpyc`, `.rpa` unpacking, a visual editor, a glossary, media localization, an AI assistant (Ollama / clipboard), a runtime patch, translation transfer between game versions.
