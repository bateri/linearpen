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
- **Enter opens an issue beside the list.** The panel shows what the list does not carry: the
  description (as markdown, with its links opening only where you press them), the comments oldest
  first, the cycle, the project and who it is assigned to. `Open in Linear` (`o`) opens it in your
  browser and `Close` (`Esc`) puts it away; the panel follows the cursor, asks Linear once for each
  issue, and takes the whole of a pane too narrow for both. Space and Shift Space scroll an issue
  longer than its room, and Enter in the filter opens the issue the cursor is on.
- **Select issues.** `x` or the box of a line selects it, Shift with the arrows selects more, and
  `Cmd A` selects every issue that is shown. `Esc` closes what is on top: the issue, then the
  selection, then the filter.
- **A command palette (`Cmd K`) and a card of the keys (`?`).** Both are made from the one table of
  commands that the key line is made from, so a key is written once; words typed in the palette that
  are no command's filter the issues. In a narrow pane the card is one column, and the arrows and
  Page Up and Page Down scroll it.
- **Work on an issue, or the ones you selected, with an agent.** `w` (or the button over the list)
  opens a form beside the list: the issues, what to tell the agent, which agent (Claude, Codex or
  Cursor, the ones in your `PATH`), whether it works in this folder or in a new `git` worktree on the
  issue's branch, and whether its pane splits to the right or opens a new tab. `Cmd Enter` starts it
  in a pane of bateri, and each issue says which agent works on it until the pane ends. The issues
  are written, with their descriptions and comments, to a file only you can read, under
  `~/.local/state/linearpen/work`; the agent is told where the file is and that other people wrote
  it, and is started with no option that lets it skip its questions. Nothing is started outside a
  bateri pane, and the form says why.
- **New issue.** `c` (or the button over the list) opens a form beside the list with a title, a
  description, and the values Linear's own form asks for: team (where there are several), status,
  priority, assignee (you, to begin with), labels and project, each a button that opens a menu of the
  others. `Cmd Enter` or `Create issue` makes it, and the list shows the issue under the cursor with
  its panel open. `Esc` keeps the draft; a refusal of Linear is said in the form with what was
  written still in it. It is the one thing linearpen writes to Linear, it is sent once under an
  identity the draft was given (an answer that was lost is asked for by it, not sent again), and it
  needs a key with Create issues permission, which the Connect screen now says.
- **`linearpen --text` prints your open Linear issues.** Two lists, My issues and All issues, each
  grouped by the state the issues are in (started, then not started, then the backlog and triage),
  the issue updated last first. It is also what linearpen prints in a terminal that shows no views.
- **It asks Linear with `curl`, and writes one thing: a new issue.** The key is read from `LINEAR_API_KEY`, or from
  the file `key` of `~/.config/linearpen`, and is given to `curl` on its stdin, never as an argument
  or in its environment. A key file that other users can read is read, and `linearpen --text` warns about it.
