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
  issue, and takes the whole of a pane too narrow for both. There the list is not on the page, so the arrows
  scroll the issue, `j` and `k` go to the issue after and the one before, and no key does something
  to a line that is not shown. Space and Shift Space scroll an issue
  longer than its room, and Enter in the filter opens the issue the cursor is on.
- **Select issues.** `x` or the box of a line selects it, Shift with the arrows selects more, and
  `Cmd A` selects every issue that is shown. `Esc` closes what is on top: the issue, then the
  selection, then the filter.
- **A command palette (`Cmd K`) and a card of the keys (`?`).** Both are made from the one table of
  commands that the key line is made from, so a key is written once; words typed in the palette that
  are no command's filter the issues. In a narrow pane the card is one column, and the arrows and
  Page Up and Page Down scroll it.
- **Work on an issue, or the ones you selected, with an agent.** `w` opens a form beside the list.
  Its button stands by what it works on: by the count of what is selected (`Work on them`), in the
  panel of the issue that is open (`Work on issue`), or at the end of the head, where it names the
  issue under the cursor (`Work on ACM-412`). The form holds: the issues, what to tell the agent, which agent (Claude, Codex or
  Cursor, the ones in your `PATH`), whether it works in this folder or makes a new `git` worktree
  first (by the project's own rules for one; where it has none, on the branch Linear suggests, which
  the file of issues gives), and whether its pane splits to the right or opens a new tab. `Cmd Enter` starts it
  in a pane of bateri, and each issue says which agent works on it until the pane ends. The issues
  are written, with their descriptions and comments, to a file only you can read, under
  `~/.local/state/linearpen/work`; the agent is told where the file is and that other people wrote
  it, and is started with no option that lets it skip its questions. Nothing is started outside a
  bateri pane, and the form says why.
- **Starting an agent can move the issues to work in progress.** The form has a `Status` row,
  `In Progress` (your team's first state of work, by its own name) or `Unchanged`, and it is not
  there when every issue has begun. Chosen, it moves the issues in Triage, Backlog or Todo to that
  state once the pane is made, one change for each team, and nothing else: a started, done or
  canceled issue is left as it is, and nothing is written if the pane was not made. A move Linear
  refuses does not stop the agent; the list says it did not happen. In a short pane the line
  under Folder is left out, so that Start stays on the page.
- **A row says when its agent has stopped.** While an agent is on, linearpen asks bateri for the
  panes every three seconds, and a pane that has written nothing for ten seconds is an agent that
  is done or waits for you (they cannot be told apart): the row says `Claude idle`, in the warning
  colour, in place of the ring and the name, and the ring is back when the pane writes. Where bateri
  does not say (a list it cannot read, a pane it does not hold) the row stays as it was; nothing is
  looked at while no agent is on.
- **A title you typed is not lost.** The title of the open issue is saved when its field loses the
  keyboard (a press anywhere else, or `Tab`) as well as on `Enter`, when it is not empty and is not
  the one it was; only `Esc` leaves the old title.
- **The form opens as you left it.** The agent, the folder, where the pane goes and `Status` of each
  start are kept in `prefs.json` in the state folder (`0600`, four words, nothing of Linear's and no
  key) and the next run opens the form with them. An agent that is no longer installed, or a worktree
  where there is no repository, is the default again; a file that is broken is read as if it were not
  there.
- **Old files of issues are put away.** When linearpen begins and after each start, the folders under
  `~/.local/state/linearpen/work` that are older than 14 days are removed, but only those linearpen
  made (named by their time, holding nothing but `issues.md`, not a link) and none whose pane this
  run of linearpen has started and watches. A folder with anything else in it stays, and nothing
  outside `work` is touched.
- **`make install`** builds the program and puts it in `~/.local/bin` (`INSTALL_DIR=` says another
  folder); `make uninstall` takes it away.
- **The sentence that asks for a worktree** says "the instructions it keeps for agents, such as
  CLAUDE.md or AGENTS.md" and no longer names one agent's file.
- **New issue.** `c` (or the button over the list) opens a form beside the list with a title, a
  description, and the values Linear's own form asks for: team (where there are several), status,
  priority, assignee (you, to begin with), labels and project, each a button that opens a menu of the
  others. `Cmd Enter` or `Create issue` makes it, and the list shows the issue under the cursor with
  its panel open. `Esc` keeps the draft; a refusal of Linear is said in the form with what was
  written still in it. It is sent once under an
  identity the draft was given (an answer that was lost is asked for by it, not sent again), and it
  needs a key with Create issues permission, which the Connect screen now says.
- **Change an issue: status, priority, assignee, labels, project and title.** The issue open beside the
  list has these as buttons, the same ones the form of a new issue has (the status offers every
  state of the team, Done and Canceled too), and its title becomes a field on `e` or a press (`Enter`
  saves, `Esc` leaves it, an empty title is not sent). From the list `s`, `p`, `a`, `l`, `Shift P` and `e` change
  the issue under the cursor, or all the issues selected, up to fifty in one request; a label that
  every one of them has is taken off, issues of several teams change only their priority, and a menu
  says how many issues it is for. A change is sent when you choose it, once, with the control (or the
  line) busy until Linear has answered; the issue goes to its new group with the cursor on it, and an
  issue the change takes out of the list (done, canceled, given to someone else) stays open beside it
  until you move the cursor. If Linear says no, the words are under the controls (or in a strip over
  the list) and the value is as it was. It writes only these fields (never the description or the
  team), by the one mutation `IssueBatchUpdate`, and needs a key with Write permission.
- **`linearpen --text` prints your open Linear issues.** Two lists, My issues and All issues, each
  grouped by the state the issues are in (started, then not started, then the backlog and triage),
  the issue updated last first. It is also what linearpen prints in a terminal that shows no views.
- **It asks Linear with `curl`, and writes two things: a new issue and a change to an issue.** The key is read from `LINEAR_API_KEY`, or from
  the file `key` of `~/.config/linearpen`, and is given to `curl` on its stdin, never as an argument
  or in its environment. A key file that other users can read is read, and `linearpen --text` warns about it.
