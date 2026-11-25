# Skills Manager (fork)

A desktop app that keeps your AI agent skills — for Claude Code, Codex,
Cursor and 50+ other tools — in one central library and links them into each
agent's skills folder, with presets, per-project workspaces and a skill
marketplace.

This is a fork of [xingkongliang/skills-manager](https://github.com/xingkongliang/skills-manager).
Everything not listed below works as in the original — see its README for
features, install instructions and documentation.

## Differences from the original

- **Local sharing instead of Git backup and sync.** The Backup page, GitHub
  sign-in, automatic backup, multi-device sync and conflict handling are
  removed. A new **Share** page exports any library skills to one `.zip` and
  imports such a zip (skills you already have are skipped). Only skill files
  are included — no account, no network.
- **Automatic adoption of agent skills.** On launch, skills found as plain
  folders in your agents' skills directories are imported into the library
  and replaced by links to it. Originals are copied to `adopt-backups/` in
  the library folder first. Toggle it, or run it on demand, under
  **Settings → Library**.
- **Create and edit skills.** A **Create skill** button (My Skills and
  Install Skills) writes a new `SKILL.md` from a template, can link it to
  agents right away, and opens it in an editor. Any library skill's
  `SKILL.md` can be edited from its detail panel; agents holding copies are
  refreshed on save.
- **Ranked search on My Skills.** Covers names, descriptions, the full
  `SKILL.md` text, tags and source; tolerates typos and partial words; takes
  `tag:` / `source:` filters.
- **Semantic categories.** Skills are sorted into 13 editable categories
  (plus "Other") by a small on-device embedding model (all-MiniLM-L6-v2,
  bundled — nothing leaves the machine). Each category's description is what
  skills are matched against. Category chips filter My Skills, the
  Leaderboard, the Collections tab and the Git install preview. Index and
  vectors are cached in `semantic.db` in the library folder.
- **Collections tab in Install Skills.** Every skill (about 2,900) from 120
  curated GitHub repositories, each with its description, category,
  search and a one-click install. The repositories themselves are listed on
  the Git tab. Both ship with the app (`src/data/`); refresh them with
  `scripts/build-skill-catalog.mjs`.
- **Descriptions on the Leaderboard.** skills.sh lists names only; the app
  ships descriptions for about 90% of the leaderboard skills, read from their
  repositories' `SKILL.md`.
- **New icon**, and bigger skill cards in Install Skills.
- **No app update checks.** The in-app updater and the startup check against
  the original's GitHub releases are removed.
- **English only.** Other UI languages and the Chinese README/CHANGELOG are
  removed.
- **CLI:** the `git` subcommand (backup) is removed.
- **Larger download:** the bundled model adds about 90 MB.

## Building

```bash
npm install
npm run tauri build   # fetches the pinned embedding model first (~90 MB)
```

No telemetry was found in the original; none is added.

## License

MIT, as the original — see [LICENSE](LICENSE).
