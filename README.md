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
- **Skill collections in Git install.** The Git tab of Install Skills lists
  122 GitHub repositories that publish skills; picking one opens the normal
  preview to choose which skills to install.
- **No app update checks.** The in-app updater and the startup check against
  the original's GitHub releases are removed.
- **English only.** Other UI languages and the Chinese README/CHANGELOG are
  removed.
- **CLI:** the `git` subcommand (backup) is removed.

No telemetry was found in the original; none is added.

## License

MIT, as the original — see [LICENSE](LICENSE).
