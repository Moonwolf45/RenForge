# Changelog

[Русский](CHANGELOG.md) · **English**

## [1.3.0]

### Added
- **Translation QA:** a single indicator in the editor header (errors / to review / all clear) with jumps to problem lines, highlighting of errors and warnings, and quick-fix buttons — strip a "stuck" AI prefix (like `[ENGINE]:`), restore lost leading tags, auto-wrap long interface strings; plus fix-the-whole-file in one click.
- **Manual "translation confirmed" mark** — for strings whose correct translation equals the original (`…`, names, numbers), so they no longer count as untranslated.
- **Duplicate marking in the editor:** strings whose original occurs several times in the project get a count badge (duplicates share one translation — translate once). If duplicates end up with different translations, a conflict warning appears: only one variant reaches the game, so they should be unified.
- **Duplicate text variants:** for games where the same line exists in different wordings (base text + `tl/english`), your translation is now delivered under all variants — translate once (all variants are shown in the editor). Fixes the "half the game in Russian, half in English" case.
- **Extraction-method tag** (AST / regex) — shows how reliably a string was extracted.
- **"Game files" modal:** an overview of all found files with their status (extracted / other language / not extracted) and opening of any source file — instead of hunting for a file in the OS explorer.
- **Batch string export** to all formats at once (CSV, JSON, PO), one file per source, with correct names.
- **Unified volume control** on the Audio tab that syncs all players.
- **Orphan `.rpy` warning** (a `.rpy` without a compiled `.rpyc`) with advice to run the game once and re-extract for an accurate result.
- **Coverage diagnostics (for testing):** the mod build can be switched to diagnostic mode — the built translation then logs text that appeared in-game but isn't covered by the translation. The "Uncovered text" report lists such strings, cross-referenced against the database (seen in-game but not extracted = candidates), and lets you add any of them to manual strings in one click. Helps find extraction and delivery gaps.
- **Multi-language collision warning:** if the target language matches a language the game already ships, the dashboard shows a warning — your translation may overlap the game's built-in localization (if the player selects that language in the game's menu). Helps avoid picking a conflicting target language by mistake.

### Changed
- **Redesigned language-pair cards:** clear status (draft / built / modified), progress and actions.
- Single-file string export now fills in the correct file name (e.g. `script.rpy.po`).
- More noticeable color indication of string status in the editor list.
- Statuses and indicators update more accurately after saving.
- **Image viewer:** the harsh transparency checkerboard was replaced with a soft theme-aware one; added a backdrop switch (checker / dark / light) to inspect transparent PNGs.
- **Carried-over and memory-filled translations now reach the game right away** (previously they waited for a manual "reviewed" mark): the built mod delivers them and flags "needs review: N" in the build report — you can verify them in-game (Shift+R), and the yellow flag stays in the editor. The build report now shows how many strings were delivered (dialogue / UI), how many need review, and how many unsafe ones were skipped.

### Fixed
- **Steam achievements are no longer blocked by the mod:** removed forced developer/console mode from the delivery patch.
- **Source viewer** no longer crashes with an encoding error on old games (Ren'Py 6.x).
- Precise scrolling and highlighting of the target line when navigating from search or the list (it could previously miss).
- RenForge's own files (patch and translations) no longer appear in the game's source-file list.
- Header tab buttons (Text / Images / Audio) are now truly centered at any window width (they used to drift right on wide screens).
- **Extraction resilience:** a parse failure on one problem file no longer aborts the whole extraction — that file is skipped (logged) and the remaining strings are extracted as usual.
- **Files skipped during extraction are now visible in the UI:** if a game file couldn't be parsed, a warning listing those files appears after extraction — previously this was only visible in the technical log.
- **Translation file robustness:** Unicode line-separator characters (U+2028/U+2029) in the text can no longer corrupt the delivery-file build — they are now escaped (no effect on the in-game result).

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
