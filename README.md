# Skills Manager (fork)

A desktop app that keeps your AI agent skills — for Claude Code, Codex,
Cursor and 50+ other tools — in one central library and links them into each
agent's skills folder, with presets, per-project workspaces and a skill
marketplace.

This is a fork of [xingkongliang/skills-manager](https://github.com/xingkongliang/skills-manager).
Everything not listed below works as in the original — see its README for
features, install instructions and documentation.

## What this fork adds

### Find skills

- **Collections tab** (Install Skills → Collections). Every skill from 120
  curated GitHub repositories — about 2,900 — as its own card with
  description, category, search and one-click install. Shipped with the app,
  browsable offline.
- **Your own collections** (Install Skills → Git → Skill collections). Paste
  any GitHub repo (`owner/repo` or URL) and click **Add**: its skills join the
  Collections tab. Rename, refresh or remove it from the same list. Stored in
  `collections.json` in the library folder.
- **Descriptions on the Leaderboard.** skills.sh lists names only; the app
  ships descriptions for about 90% of leaderboard skills, read from their
  repositories' `SKILL.md`.
- **Ranked search on My Skills.** Searches names, descriptions, the full
  `SKILL.md` text, tags and source; tolerates typos and partial words; takes
  `tag:` and `source:` filters.

### Organize

- **Semantic categories.** Skills are sorted into 13 editable categories
  (plus "Other") by a small embedding model that runs on your machine
  (all-MiniLM-L6-v2, bundled — nothing is sent anywhere). Each category's
  description is what skills are matched against; edit it to re-sort.
  Category chips filter My Skills, the Leaderboard, the Collections tab and
  the Git install preview.
- **Automatic adoption.** On launch, skills found as plain folders in your
  agents' skills directories are imported into the library and replaced by
  links to it, so one copy serves every agent. Originals are backed up to
  `adopt-backups/` first. Toggle or run it under **Settings → Library**.

### Create

- **Create skill** (My Skills and Install Skills). Writes a new `SKILL.md`
  from a template, optionally links it to agents, and opens it in the editor.
- **Edit SKILL.md** from any skill's detail panel (pencil icon). Linked
  agents see the change at once; agents holding copies are refreshed.

### Share — locally, instead of Git sync

- **Skills:** the **Share** page exports any library skills to one `.zip`
  and imports such a zip; skills you already have are skipped.
- **Collections:** export the repos you added as collections, or import
  someone's list (our JSON file, or a plain list of `owner/repo` lines).
- No account and no network involved — pass the file on any way you like.

### Removed

- Git backup and multi-device sync (Backup page, GitHub sign-in, automatic
  backup, conflict handling) and the CLI's `git` subcommand.
- App update checks: the in-app updater and the startup check against the
  original's releases.
- Other UI languages and the Chinese README/CHANGELOG — English only.

No telemetry was found in the original; none is added. The bundled model
makes the download about 90 MB larger. Search index, vectors and category
edits live in `semantic.db` in the library folder.

## Building

```bash
npm install
npm run tauri build   # fetches the pinned embedding model first (~90 MB)
```

## License

MIT, as the original — see [LICENSE](LICENSE).
