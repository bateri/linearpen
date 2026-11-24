# Changelog

## [Unreleased]

### Added

- **Size the issue.** Drag the edge between the list and the issue (or press `Cmd Shift Left` and
  `Cmd Shift Right`) to make the issue wider or narrower. linearpen opens it as wide the next time.
- **One click opens an issue.** A line of the list opens its issue (or project) beside the list with
  one click, as in Linear, not two; a double click out of habit does not press what opens under it.
- **A form gives way to the line you choose.** With the form of Work on or of a new issue open,
  pressing an issue in the list puts the form away and opens that issue; what you wrote for the agent is
  kept for its issue, and the draft of the new issue as it was.
- **The inbox tab says when something is unread.** A dot on Inbox while your inbox has an unread
  notification of an issue (asked with each refresh), and the card's Inbox line says so too.
- **Try again has its key.** Every Try again (an issue, the choices of a new issue, a project, a pull
  request) says `Cmd R`, which asks again. Going back from a pull request stops `gh` asking GitHub.
- **Tabs on a wide pane.** The head has My issues, All issues, Inbox and Projects as tabs, `1` to `4`,
  and More after them for every other list. A narrow pane keeps the list's button. The first five
  favorites are now `5` to `9`.
- **Projects.** The workspace's projects are a list of the card of lists (`g`), as Linear's Projects
  page has them: by their state, each with its health (a shape and a word), its lead, its target and
  how far it is. Enter opens one beside the list with its last update, whole, and its milestones;
  `Shift I` (Show issues) goes to its issues and `Esc` comes back. Read only.
- **Inbox.** Your Linear inbox is a list of the card of lists (`g`): the issues of your notifications,
  the unread ones first, each saying who and what it is about (`Deniz · Mention`). Open one beside it,
  change it or work on it as from any list. Opening an issue marks nothing read; press `u` (or Mark
  read in its menu) to mark its notifications read in Linear.
- **The pull request of an issue.** An issue with a pull request of GitHub linked to it shows it under
  the issues round it. Press it (or `Shift G`) and the pull request opens in the place of the issue:
  its state, its branch, its checks (what failed and what still runs), its review and its files, with
  `Cmd Shift O` to open it on GitHub and `Esc` back to the issue. It is read with `gh`, GitHub's own
  command, signed in as you; where `gh` is not installed it opens in your browser, as a link.
- **Created by me and Subscribed.** Two more lists in the card of lists (`g`), as Linear's My issues
  has them: the issues you made, and the issues you are subscribed to.

### Changed

- **The issue opens from the top of the pane.** The list's head (the list button, the filter, Refresh
  and New issue) stays over the list, and no longer runs over the issue. Where the list has too little
  room beside the issue, Refresh and New issue are in the line of keys; on a pane under 867 points the
  issue has the pane, as on a narrow one.
- **Refresh with a form open.** `Cmd R` asks Linear again while the form of a new issue or of Work on
  is open, and the line of keys says so where the head has no room for the button.
- **Smoother motion.** The issue fades in as it opens, a group's arrow turns as it folds, the row of
  filters opens and closes in place, and the dot of an agent that has stopped breathes slowly. The line
  that says Linear is asked again is moved by the terminal and costs the page nothing. This needs bateri
  0.33.0 or later; an older one shows them as before.

### Fixed

- A screen reader, and an agent that reads the pane, are told which value's card is open (the labels
  and the filters), and what each value chip holds (the status, the priority, the labels, a filter's
  choices) beside what it is. They were told by the ring around it, and of the chip only its name.
  The issue's title, in the pane that shows the issue, is read as the title (the button that edits it was
  named "Edit the title", and the title itself was not read).

## [0.7.0] - 2026-11-20

### Added

- **Share an issue.** Press `Share` next to an issue's identifier (or `Cmd Shift S`) to copy its ID,
  title and link as one line, ready to paste into a chat. It is in the right-click menu too.
- **Go to the agent.** Press `Go to Claude` on an issue (or `Shift W`, or Go to the agent in its menu)
  and bateri takes you to the pane of the agent that works on it.
- **Agents are found again.** Begin linearpen again and the issues agents still work on have their
  marks back, including agents another linearpen started.
- **Agents on the command line.** `linearpen list` and `show` say which agent is at work on an issue,
  and in which pane; `linearpen agent ACM-412` takes you there.

### Changed

- **Claude is named for its issue.** Work on starts Claude with the session named `ACM-412 - Add rate
  limiting`, the issue's identifier and title, so its prompt box, `/resume` and the terminal's title
  say which issue it works on. This needs Claude Code 2.1.76 or later.
- **Pick several labels and filter values at once.** Labels (on an issue, a new issue or a selection)
  and the five filters open a card that stays open while you check and uncheck values, with a search
  field to find one. In a bateri pane this needs the next bateri; until then they open the old menu,
  one pick at a time.
- **The agent's mark says where it goes.** It reads `Go to Claude`; an agent that has stopped writing
  has an amber dot in place of the ring, where the row said `Claude idle`.

### Fixed

- **Filters open on the first click.** Clicking a filter right after linearpen started did nothing.
- **A click is no longer lost while the filter row is open.** Clicking an issue's labels or status then
  only closed the row.
- **Refresh shows in every pane.** While Linear is asked again, a thin line runs along the top of the
  page, also where the pane is too narrow for the Refresh button.
- **Refresh reads the open issue again.** Its description and comments come again from Linear, and so do
  those of an issue you opened earlier, the next time you open it.
- **Refresh reaches the New issue form.** A team, label, project or person made in Linear is offered in
  the form after Refresh, even with a draft already begun; restarting linearpen is no longer needed.

## [0.6.0] - 2026-11-19

### Added

- **Edit descriptions and comments.** `Edit description` (`E`) and `Edit comment` on your own comments
  (`Cmd Shift M` edits your last one) turn the words into a field; `Save` (`Cmd Enter`) writes them. If
  someone changed them in Linear meanwhile, nothing is overwritten until you save again.
- **The same on the command line.** `linearpen update KEY-N --description -` writes a description, and
  `linearpen comment KEY-N --edit last --body -` edits your last comment; an agent started with Work on is
  told both.

### Changed

- **Every status is in the lists.** My issues, All issues and the lists of a team, a cycle, a view, a
  project or a label hold the 100 done and canceled issues that changed last, folded at the end, and
  the Status filter and `linearpen list --status` take every status. The list button still counts the
  open issues. A team's list is now called `Issues`.

### Fixed

- **One click runs a command in the palette.** A row under the mouse needed a double click; a single
  click now does it. In a pane, this comes with the bateri that carries the fixed palette.

## [0.5.0] - 2026-11-19

### Added

- **Files in issues.** The pictures in a description or a comment are drawn where they are written, and
  a click opens one over the pane. Any other file is a card with the icon of its kind, its name and one
  verb, `Save` (it is written where you choose, up to 100 MiB); a picture that is not drawn says why. The
  rest of what you can do with a file (open its link, copy it) is in its right-click menu. A picture of an
  address that is not Linear's is a link that opens in your browser. You can add any file up to
  100 MiB to a comment, to a new issue or to the end of an issue's description with `Attach file`
  (`Cmd Shift U`), by pasting or by dropping it.
- **PDFs, read in the pane.** A PDF's card has `Open`, which shows it over the pane a page at a time
  (`Save` and `Close` are at the top, and a key is written on its button).
  Select its words with the pointer or with `Shift` and the arrows, copy them with `Cmd C`, and look for
  words with `Find` (`Cmd F`): the matches are marked on the page and `Enter` goes from one to the next.
- **Files on the command line, and for the agent.** `linearpen show` prints each picture and file as
  `[image N: name]` or `[file N: name]` and `--files DIR` saves them. `--attach PATH` adds a file to a
  `comment`, a `create` or the end of a description with `update`. The file Work on writes for an agent
  says when its issue has files and how to get them.

### Fixed

- Files added while others are being sent stay in the tray, and `show --files` saves at most 100 files
  and 1 GiB.

### Changed

- **The agent is no longer held to its own issue.** linearpen does not refuse a write from the pane
  Work on opens: the agent runs the same commands you do, with your key. What it asks you before
  running a command is its own question, as before. The prompt now says what `linearpen` can do for
  it.

## [0.4.0] - 2026-11-19

### Added

- **Give the agent a prompt from Work on.** Words you write in the form's `Prompt` box, such as
  "Release it when the gate is green", go to the agent with the issue, as your own.

### Changed

- **New worktree works in any folder.** Started in a folder that holds several repositories, the
  agent makes the worktree of the one the issue is about.

## [0.3.1] - 2026-11-18

### Changed

- **In a narrow pane, an issue or a form has the pane to itself.** When an issue, New issue or Work
  on fills a narrow pane, the list's name and the filter no longer sit above it, and their keys
  (`g`, `1`, `2`, `/`, `f`) wait until you are back at the list. Close it and they are there as you
  left them.

## [0.3.0] - 2026-11-18

### Added

- **A terminal that speaks Kitty graphics is drawn on, as bateri does.** linearpen shows its issues
  as pixels there, with the mouse and the keys; a terminal with no Kitty graphics, and a multiplexer,
  keeps what linearpen printed. The keys are those of a keyboard with no ⌘: Ctrl is what ⌘ is
  (copy in a field is Ctrl+Shift+C; Ctrl+C ends linearpen).
- **Each release has two files for a machine.** A small one, which bateri and kablo install, and one that draws in any terminal that speaks Kitty graphics, which `install.sh` installs.

## [0.2.0] - 2026-11-17

### Added

- **Use linearpen from the command line.** You no longer need the interface to look at or change an
  issue: `linearpen list` prints your issues, `linearpen show ACM-412` prints one with its
  description and comments, and `update`, `create` and `comment` change them. Statuses, labels and
  people are written by the names you see in Linear, as in
  `linearpen update ACM-412 --status "In Review" --label bug`. If a name does not match, linearpen
  sends nothing and lists the names that would. Add `--json` when a script reads the output, and
  run `linearpen help` for the rest.
- **The agent can report back.** When you start Claude, Codex or Cursor on an issue with Work on,
  it is told it may move that issue to another status and leave a comment on it once it is done or
  stuck. That is all it can write: from the agent's pane, linearpen refuses to change any other
  issue or anything else. This stops an agent from doing more than you asked, for example because a
  comment on the issue told it to. It is a guard, not a lock, and the agent still asks you before it
  runs a command. What the agent writes appears in Linear under your name.
- **Drag an issue out of the list.** Drop it on a terminal pane, for example one running Claude Code,
  and its ID, title and link are typed there. Nothing runs until you press Enter.

## [0.1.0] - 2026-11-12

### Added

- **Filters and search.** `f` opens a row of five filters under the head (Status, Priority, Assignee,
  Labels, Project): the keyboard is on the first chip, `Tab` goes on, `Enter` opens its menu (the states
  by name, the people with you first, `No assignee`, the labels, the projects, `No project`), each pick
  adds a value or takes it away, so a chip holds several (any of them) and the chips together are all
  required. With the keyboard back on the list only the chips that are set stay, with `Clear filters`
  (`Shift F`); with none set there is no row. Linear does the narrowing (`state.name in`, `priority in`,
  `assignee.isMe`, `assignee.id in`, `assignee null`, `labels.some.name in`, `project.id in`,
  `project null`), put with the list's own filter, in every list; they are forgotten when the pen
  closes. While you type in the filter field, `Cmd Enter` searches all of Linear (`searchIssues`:
  the whole workspace, closed issues too, up to 500, closed groups folded): the result is the list
  **Search**, the words stay in the field, the filters apply, and `Esc` goes back to the list you were
  in. Where nothing in the list matches, the page says `Nothing in <list> matches` and has a button for
  the same search. `view` takes `filters` and `list: { kind: "search", id: <words> }`, `filter_options`
  tells what the menus offer, and the topic tells `filters` (`docs/calls.md`). Both read only.
- **Lists.** The card of lists (`g`, or the list button) goes to more than My issues and All issues:
  what you starred in Linear (the keys `3` to `9` for the first seven), the open issues and the running
  cycle of the teams you are in (in the order of your own list of teams), saved views of issues and projects, each line saying which it is. Typing finds
  a list by name. The list button says the list and how many issues it has (`500+` where Linear has
  more than the 500 that changed last, which a line at the end of the list says too). Only the list shown
  and My issues are asked for, so All issues waits until it is shown, and the names of the lists are
  asked for with them (they are no more than names: a failure leaves My issues and All issues, and
  says so on the card). The cards of lists and of commands keep inside a short pane. `view` takes `list: { kind, id }` instead of `scope`; the topic tells
  `listId`, `listing`, `counts.shown` and the `catalog` (`docs/calls.md`).
- **Comments.** Under the comments of the issue open there is a field to write one (`Shift M` gives
  it the keyboard; the keys are the field's while it has it, `Cmd Enter` posts, `Esc` leaves it).
  Comment posts the words as they were written, once, with one more mutation, `CommentCreate`; the
  comment is among the others when Linear has it, and the field is empty. What is written stays for
  the issue in memory, whichever issue is open, and is never sent by itself. If Linear does not post
  it, the words say so under the field and what was written stays; if no answer came, the words say
  that it may have been posted, and nothing is sent again. A comment is up to 10 000 characters.
- **Cycle, estimate and due date.** The issue open has a button for each (`Cycle 12`, `M`, `Nov 15`,
  or their names, dim, where it has none), and the keys `Shift C`, `Shift E` and `Shift D` change
  them for the issue under the cursor or for all that are selected; the bar of the selection has
  Cycle as its sixth value in a wide pane, and a right press lists all three. The cycle menu has the
  cycle that runs, the next six and `No cycle`; the estimate menu is the scale of the team (exponential,
  Fibonacci, linear or t-shirt sizes, which are sent as Fibonacci numbers; 0 only where the team
  allows it) and `No estimate`; the due date menu is Today, Tomorrow, In 3 days, In a week, In 2
  weeks and `No due date`, by your own day. A team with no cycles, or that does not estimate, has no
  such button or menu item. `change` takes `cycle`, `estimate` and `dueDate`, each alone.
- **The issues round an issue.** Under its values the issue open lists its parent, its sub-issues and
  what it is tied to (`Blocks`, `Blocked by`, `Related`, `Duplicate of`, `Duplicated by`), each a line
  that opens that issue (beside the list, as it is, where the list does not hold it); the file Work
  on writes for the agent tells them too, under the same frame. They are read only. `Cmd Shift Up`
  opens the parent and `Cmd Shift Down` opens a sub-issue or a related issue (a menu, each line with
  its way, or straight to the one there is), as in Linear; both are in the palette and on the card
  of keys.
- **The head of the page has fixed places.** The list button (the list, how many it has and a
  chevron) opens a card of lists, which for now holds My issues and All issues (`g`, or `1` and
  `2`); the filter says how many it lets through (`4 of 14`) inside itself, and Refresh and New
  issue stay where they are whatever the page is doing: the cursor moving, issues selected, an issue
  open or shut. A page too narrow for Refresh (under 880 points) or for New issue (under 760) leaves
  their buttons out; the keys still work. Down to 540 points the head is one row, the list at the
  left and the filter at the right, as wide as in a wide pane whatever the list is called (a long name
  is cut in its button); under 540 the filter takes a row of its own.
- **The bar of the selection.** With issues selected the line under the page is a bar: how many are
  selected, the values to change in them (Status, Assignee, and in a wider pane Priority, Labels and
  Project), `More` (`Cmd K`) and `Clear` (`Esc`); a press on a value opens its menu for all of them,
  and `Esc` gives the line of keys back. The head has no count and no Work on button any more.
- **A right press on a line opens its menu.** The values (Status, Priority, Assignee, Labels,
  Project), `Assign to me` (`i`), and for one issue `Rename` (`r`), `Copy ID`, `Copy branch name`,
  `Copy link` and `Open in Linear`; on a line that is selected it is the menu of the whole
  selection, without what is of one issue, and the cursor goes to the line that was pressed.
- **Copying.** `Cmd Shift I` puts the identifier of the issue (the open one, the one selected or the
  one under the cursor) on the pasteboard, `Cmd Shift B` its branch name and `Cmd Shift L` its link,
  and the page says what it copied for a moment. The page gets the branch name and the link of each
  issue with the rows (`branchName` and `url`).
- **Linear's keys.** `r` renames, `o` opens the issue (Enter's twin), `Shift O` opens it in Linear,
  `Cmd R` asks Linear again, `i` assigns the issue to you and `g` goes to a list. Keys that were
  `e` (rename), `o` (open in Linear) and `r` (ask again) are not those any more. With an issue open,
  `r`, the copies and `Shift O` are that issue's whatever is selected (they were nobody's with
  several selected, and another issue's with one); the keys that change a value stay with the selection.

- **linearpen opens in a bateri pane as a list of your issues.** My issues and All issues, each
  with its count, grouped by state with the backlog folded; the arrow keys and `j` and `k` move,
  Left and Right fold a group, `g` (or `1` and `2`) chooses the list, `/` filters by title,
  identifier or label, and `Cmd R` asks Linear again. A pane that is narrow drops the labels and the
  buttons of the head.
- **Connect, loading, error and empty screens.** Without a key the pane asks for one (the field
  hides what you type) and keeps it where `LINEAR_API_KEY` is not set; a key Linear refuses is
  said there, and one Linear cannot be asked about leaves a way to try again.
- **Enter opens an issue beside the list.** The panel shows what the list does not carry: the
  description (as markdown, with its links opening only where you press them), the comments oldest
  first, the cycle, the project and who it is assigned to. `Open in Linear` (`Shift O`) opens it in your
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
- **Work on an issue, with an agent.** Work on is one issue's, in its detail, at the start of its
  first line (`Work on issue`, `w`), as in Linear; the list has no button for it and a selection
  starts no agent. The form holds: the issue, which agent (Claude, Codex or Cursor, the ones in your
  `PATH`), whether it works in this folder or makes a new `git` worktree first (by the project's own
  rules for one; where it has none, on the branch Linear suggests, which the file of the issue gives),
  and whether its pane splits to the right or opens a new tab; it has no words for the agent, which
  is written to in its own pane. `Cmd Enter` starts it in a pane of bateri, and the issue says which
  agent works on it until the pane ends. The issue is written, with its description and comments, to
  a file only you can read, under `~/.local/state/linearpen/work`; the agent is told where the file
  is and that other people wrote it, and is started with no option that lets it skip its questions.
  Nothing is started outside a bateri pane, and the form says why. In a pane on a server over `ssh`
  the agent, its pane and the file are on the server.
- **Starting an agent can move the issue to work in progress.** The form has a `Status` row,
  `In Progress` (your team's first state of work, by its own name) or `Unchanged`, and it is not
  there when the issue has begun. Chosen, it moves an issue in Triage, Backlog or Todo to that
  state once the pane is made, and nothing else: a started, done or canceled issue is left as it
  is, and nothing is written if the pane was not made. A move Linear
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
  state of the team, Done and Canceled too), and its title becomes a field on `r` or a press (`Enter`
  saves, `Esc` leaves it, an empty title is not sent). From the list `s`, `p`, `a`, `i` (assign to
  you), `l`, `Shift P` and `r` change the issue under the cursor, or all the issues selected, up to fifty in one request; a label that
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
- **It asks Linear with `curl`, and writes three things: a new issue, a change to an issue and a comment.** The key is read from `LINEAR_API_KEY`, or from
  the file `key` of `~/.config/linearpen`, and is given to `curl` on its stdin, never as an argument
  or in its environment. A key file that other users can read is read, and `linearpen --text` warns about it.
