# Changelog

## [Unreleased]

### Added

- **linearpen opens in a bateri pane as a list of your issues.** My issues and All issues, each
  with its count, grouped by state with the backlog folded; the arrow keys and `j` and `k` move,
  Left and Right fold a group, `1` and `2` choose the list, `/` filters by title, identifier or
  label, and `r` asks Linear again. A pane that is narrow drops the labels and the button.
- **Connect, loading, error and empty screens.** Without a key the pane asks for one (the field
  hides what you type) and keeps it where `LINEAR_API_KEY` is not set; a key Linear refuses is
  said there, and one Linear cannot be asked about leaves a way to try again.
- **`linearpen --text` prints your open Linear issues.** Two lists, My issues and All issues, each
  grouped by the state the issues are in (started, then not started, then the backlog and triage),
  the issue updated last first. It is also what linearpen prints in a terminal that shows no views.
- **It only reads, and talks to Linear with `curl`.** The key is read from `LINEAR_API_KEY`, or from
  the file `key` of `~/.config/linearpen`, and is given to `curl` on its stdin, never as an argument
  or in its environment. A key file that other users can read is read and warned about.
