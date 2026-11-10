# Changelog

## [Unreleased]

### Added

- **`linearpen --text` prints your open Linear issues.** Two lists, My issues and All issues, each
  grouped by the state the issues are in (started, then not started, then the backlog and triage),
  the issue updated last first. It is also what linearpen prints in a terminal that shows no views.
- **It only reads, and talks to Linear with `curl`.** The key is read from `LINEAR_API_KEY`, or from
  the file `key` of `~/.config/linearpen`, and is given to `curl` on its stdin, never as an argument
  or in its environment. A key file that other users can read is read and warned about.
