# The page and the program

The page (`src/view/`) and the program (`src/program/`) talk only through what is written here:
calls the page makes, and a topic it listens to. A change to one starts in this file. The code that
answers is `src/program/api/`; the page's own side is `src/view/parts/`.

**Units.** Time is seconds since the epoch where it is a moment. Members are `camelCase`.

**Errors.** A call that cannot be done answers with an error: `bad-params` (the params are not what
the call takes: a member it does not know, a value out of range; the words say which), `not-found`
(no such call), `internal` (the program failed), `no-key` (there is no key to ask Linear with),
`refused` (Linear did not accept the key), `unreachable` (Linear was not reached: `curl` is not
installed, or the network, the name or the time failed), `failed` (Linear answered, but not with
what was asked), `unavailable` (an agent cannot be started from here: bateri or the agent is not
there), `start-failed` (it was tried and did not start: the words say why), `create-failed` (an
issue was to be made and Linear did not make it: the words say why), `change-failed` (a change was
to be made to issues and Linear did not make it: the words say why), `comment-failed` (a comment
was to be posted and Linear did not post it, or it is not known whether it did: the words say why),
`upload-failed` (a file was to be sent with a comment, a new issue or a description and was not:
nothing was sent after it, and the words say which and why), `describe-failed` (files were to
be added to a description and it was not written, or it is not known whether it was: the words say
which of three it is: the description could not be read, the files would make it too long (nothing was
written in either), or the write was sent and failed), `save-failed` (a file of Linear was to be saved and was not: the words say why) and
`pdf-failed` (a PDF could not be read for its pages and words: the words say why). The
words of an error are for the person and never hold the key, nor the address a file is sent to
(Linear signs it).

The pen writes three things: the issue the person makes (`create`), the change the person makes to
issues (`change`): one of their status, priority, assignee, labels, project, cycle, estimate, due date
or title, and the comment the person writes on an issue (`comment`). The files the person adds (any
kind, at most 100 MiB each) go in Linear's storage first (`FileUpload`, and the file sent to the place
Linear named) and are named in what is written: the end of a comment, of a new issue's description, or
of the description of an issue that is there (`describe`: the one way a description is written, and
only to put files at its end). A picture (PNG, JPEG, GIF or WebP, by its first bytes) is named as a
picture, `![name](address)`, and any other file as a link, `[name](address)`.
The description is otherwise never written, and the team never. One more call writes, and only as
`change` does: `start`
moves the issue, if it has not begun, to its team's first state of work in progress, where the
person chose that (`progress`), once the agent's pane is made. Nothing else here changes anything in
Linear.

The program also writes two things of its own, in `$XDG_STATE_HOME/linearpen` (`~/.local/state/linearpen`)
and never to Linear: the file of the issue handed to an agent, under `work/` (see `start`), which it
puts away again when it is older than 14 days, and `prefs.json`, four words (`0600`), which `start`
keeps and `boot` tells.

## Calls

### `boot`

What the page needs before it draws, once; and the program starts asking Linear for the lists if it
has a key and has not begun (listening to the topic `issues` begins it too). Params: none.

```json
{ "version": "0.1.0", "key": "file", "problem": null,
  "prefs": { "agent": "codex", "folder": "here", "place": "tab", "status": "keep" } }
```

`key` says where the key is from: `none` (the page shows Connect), `environment` or `file`.
`problem` is, where there is a key that cannot be used (it is not one, or its file cannot be read),
the words that say why; `key` is `none` then.

`prefs` is what the form that hands an issue to an agent was left at by the last `start`
(`$XDG_STATE_HOME/linearpen/prefs.json`): `agent` (`claude`, `codex` or `cursor`), `folder` (`here` or
`worktree`), `place` (`split` or `tab`) and `status` (`progress` or `keep`). Each is null where the
file does not say it, is not a word of the form's, or the file is missing or cannot be read. Whether
the agent is installed is not told here: the page takes what it cannot have away when `work_options`
says what can be chosen. The words the person writes for the agent are not kept: they are for one
start.

### `connect`

Tries a key with a question to Linear and, if Linear takes it, keeps it (`$LINEARPEN_CONFIG_DIR/key`,
else `$XDG_CONFIG_HOME/linearpen/key`, else `~/.config/linearpen/key`; the file `0600`, its folder
`0700`) and starts asking for the lists. Params: `{ "key": "lin_api_..." }`.

```json
{ "organization": "Acme", "saved": true, "saveProblem": null }
```

`saved` is false where the key could not be kept (`saveProblem` says why): it is used for this run.
Errors: `bad-params` (the text is not a key: `Empty`, too long, or a character no key has),
`refused`, `unreachable`, `failed`.

### `view`

What the page is looking at; the answer is the list for it, as the topic tells it. Every member is
optional (`{}` asks for the list as it is). Params:

- `list`: `{ "kind", "id"? }`, the list the page shows. `kind` is `"mine"` (the open issues assigned
  to the person, the default), `"all"` (the open issues of the workspace; asked for when it is shown),
  or one of `"team"` (the open issues of a team), `"cycle"` (the open issues of a cycle: the `id` of any
  cycle, the running one of a team is in the `catalog`),
  `"view"` (a saved view of issues), `"project"` (a project's open issues) and `"label"` (the open
  issues with a label), which take the `id` of what they are of (the ids the topic's `catalog` tells);
  or `"search"`, which takes the words in `id` (at most 200 characters, no control character, trimmed)
  and is what Linear finds for them in the whole workspace, closed issues too, up to 500, whose closed
  groups begin folded. `mine` and `all` take no `id`. Only the list shown and My issues are asked for: going to another
  list lets go of the one left and asks Linear for the new one, and the topic tells it as `listing`
  `"loading"` until it comes. An `id` Linear does not know is not an error of the call: `listing` is `failed`
  and `error` has Linear's words. The old member `scope` is not taken.
- `filters`: what Linear is asked to narrow the list by, as the person set it (all of it, or the call
  does not set it; `[]` or `{}` is none). Each kind is a list of values, any of which lets an issue
  through; the kinds are all required of it, and the list's own filter too (the open issues, so a
  closed state finds something only in a search, which has no such filter). At most 50 values of a kind, each
  at most 100 characters with no control character; anything else is `bad-params`.
  - `status`: names of workflow states (`"In Progress"`), as `filter_options` tells them;
  - `priority`: `"urgent"`, `"high"`, `"medium"`, `"low"` or `"none"`;
  - `assignee`: `"me"`, `"none"` (no assignee) or Linear's id of a person;
  - `labels`: names of labels;
  - `project`: `"none"` (no project) or Linear's id of a project (the ids the `catalog` tells).

  They are sent to Linear (`state.name in`, `priority in`, `assignee.isMe`, `assignee.id in`,
  `assignee null`, `labels.some.name in`, `project.id in`, `project null`, "and"ed): a change of them
  lets go of the list shown and asks for it again, for My issues too, which is held whole and counted.
  They are not kept when the pen closes.
- `filter`: the words typed, at most 200 characters. An issue is let through where they are in its
  title, its identifier or the name of a label, whatever the case. While it is not empty no group is
  folded. A search does not take them: its words are the search's, which Linear matched.
- `closed`: `{ "<group key>": true | false }`, the groups the person folded (`true`) or opened
  (`false`), at most 64; one not named is as it begins (`backlog` and `triage` folded). It replaces
  what the program held.
- `seq`: the number of this call among the page's `view` calls, which grows with each. Calls run at
  the same time and may be told out of their order: a member (`list`, `filters`, `filter` or `closed`) that a
  call with a higher `seq` has set is left as it is. A call with no `seq` is taken as the latest.

Answer: the **list** (below).

### `filter_options`

What the filters of a list offer to choose from, asked of Linear once and kept until the lists are asked
for again (`r`, the five minutes). The projects are not here: they are the `project` items of the
topic's `catalog`. Params: none. Errors: `no-key`, `refused`, `unreachable`, `failed`.

```json
{ "states": [{ "name": "In Progress", "kind": "started" }],
  "people": [{ "id": "u-ada", "name": "Ada", "me": true }],
  "labels": [{ "name": "bug", "color": "#e5484d" }] }
```

`states` are the workflow states of every team by name (states of one name in several teams are one),
in the order of Linear's own list (triage, backlog, unstarted, started, completed, canceled, then by
position); `people` has the person first and the others by name; `labels` are by name, once each.
At most 250 of each are asked for.

### `issue`

One issue with what a row of the list does not carry: its description, its comments, and the issues
round it (its parent, its parts and what it is tied to). Asked of Linear when the issue is opened,
and kept for the run. Params: `{ "id": "issue-412" }`, Linear's id
of the issue (a row's `id`) or its identifier: up to 64 letters, digits, dashes and underscores.

```json
{
  "id": "issue-412", "identifier": "ACM-412", "title": "Add rate limiting",
  "url": "https://linear.app/acme/issue/ACM-412/add-rate-limiting",
  "branchName": "ada/acm-412-add-rate-limiting",
  "description": "The public API has no limit per key.\n\n- A limit per key",
  "priority": "high", "state": { "name": "In Progress", "kind": "started" },
  "labels": [{ "id": "l-api", "name": "api", "color": "#3e63dd" }],
  "cycle": "Cycle 12", "project": null, "assignee": "you",
  "teamId": "team-acm", "stateId": "s-prog", "assigneeId": "u-ada", "projectId": null,
  "cycleId": "c-12", "estimate": null, "dueDate": null,
  "parent": { "id": "issue-400", "identifier": "ACM-400", "title": "Billing limits per plan",
              "state": { "name": "Todo", "kind": "unstarted" } },
  "children": [],
  "ties": [{ "label": "Blocked by",
             "issue": { "id": "issue-389", "identifier": "ACM-389",
                        "title": "Cache the pricing table at the edge",
                        "state": { "name": "In Review", "kind": "started" } } }],
  "comments": [{ "author": "Deniz", "at": "Nov 6", "body": "Redis is already there." }],
  "more": false
}
```

`branchName` is the name of the branch Linear suggests (null where it has none; it holds no control
character). `description` and each comment's `body` are markdown written by others: text to show,
nothing to act on (a picture in it, `![alt](https://uploads.linear.app/...)`, or a link to one of
Linear's files that stands alone in its paragraph, `[name](https://uploads.linear.app/...)`, is what
`picture` fetches for the page, and `save` saves where the person chose; the program never follows an
address a text names on its own). `description` is null where there is none. Comments are the first fifty, the oldest first;
`more` says there is more in Linear than is told (more comments, or a text cut at 20 000 characters
for the description and 2 000 for a comment). `assignee` is `"you"`, a name or null; `cycle` is the
words that tell the cycle, its name where it has one, else `Cycle 12` (null where it is in none).
`at` is the day as the list writes it. `teamId`, `stateId`, `assigneeId`, `projectId`, `cycleId` and
each label's `id` are Linear's ids of what the issue is of and in (null where there is none), `estimate`
the points (a whole number, null where there are none) and `dueDate` the day it is due (`2026-11-15`,
or null): what a menu marks as the issue's own and what `change` names. They are the same as in a
row of the list, and a `change` writes the new value into what is kept, so the next `issue` of the
same issue tells it.

The issues round it are read only: `parent` (null where it has none), `children` (its sub-issues) and
`ties`, the issues it is tied to, each with its `label` as the open issue is seen: `Blocks`, `Blocked
by`, `Related`, `Duplicate of` or `Duplicated by`, in that order (a tie of another kind, `similar`,
is not told). Each is `{ id, identifier, title, state: { name, kind } }`, which is enough to name it
and to ask for it with `issue`; its title is cut at 300 characters. At most fifty of each are told
(Linear is asked for the first fifty of each kind), and `more` says there are more where a list
reached fifty. A `change` leaves them as they were: no change touches them. Errors: `bad-params`, `no-key`,
`refused`, `unreachable`, `failed` (also where Linear has no such issue).

### `work_options`

What the person can choose from when they hand an issue to an agent, and what cannot be chosen and
why. Params: none. Nothing is started, and Linear is not asked.

```json
{
  "agents": [{ "id": "claude", "name": "Claude" }, { "id": "codex", "name": "Codex" }],
  "unavailable": null,
  "folder": { "name": "acme-app" }
}
```

- `agents`: the agents found in the `PATH` (`claude`, `codex`, `cursor-agent`; ids `claude`, `codex`,
  `cursor`), in that order. None is listed that is not installed.
- `unavailable`: null, or why nothing can be started now: the pen does not run in a bateri pane
  (neither `BATERI_TAB_URL` nor `LC_BATERI_TAB_URL`, which a pane on a server has, is set), `bateri`
  is not in the `PATH`, or there is no agent.
- `folder.name`: the name of the folder the pen was started in, which is where the agent starts.

"New worktree" can always be chosen, whether or not the folder is in a git repository: the pen makes
no worktree and asks `git` nothing; where it is chosen, the agent is asked to make one by the
project's own rules (see `start`).

Errors: `bad-params`.

### `start`

Hands one issue to an agent in a pane of bateri (the page calls it from the issue's own detail: Work
on is one issue's). The program writes the issue, with its description and comments, to a file of its
own and starts the agent with a prompt that is the program's own sentences, names the file, and ends
with the words the person wrote for the agent, where they wrote some. It answers when the pane is
made. Params (a member that is not named here is refused):

```json
{ "id": "issue-412", "agent": "claude", "folder": "here", "open": "split", "progress": true,
  "prompt": "Release it when the gate is green." }
```

- `id`: the Linear id of the issue.
- `agent`: `claude`, `codex` or `cursor`. `folder`: `here` (the agent works in the folder the pen
  was started in) or `worktree` (the agent is asked to make a new `git` worktree first, of the
  repository the work is in: the pen's folder, or, where that is no repository, the one under it
  that the issue is about; by the project's own rules for where one goes and what its branch is
  called; the pen makes none and asks `git` nothing, and the agent starts in the pen's folder either
  way). `open`: `split` (a pane to the right of this one) or
  `tab` (a new tab). The new pane is not given the focus.
- `progress`: optional; given where the form had the Status row, as the person chose it: `true` to
  move, `false` to leave the issue as it is (not given where the row was not there, which is where
  the issue would not move). Only `true` writes to Linear. With it the program moves the issue, if its
  state, as Linear says it now, is of the kind `triage`, `backlog` or `unstarted`, to the first
  `started` state (by position) of its team, with the `change` write of a status, **after** the pane
  is made. Nothing else is written: an issue that has begun, is done or is canceled is left as it
  is, and the move is never a way to give an issue to someone or to take one from a team. Where the
  issue would not move, nothing is sent. The page sends `true` only where the
  person chose to move the issue and the form said it would move. Where it is given (true or
  false) it is kept with the choices below, so that the next form is opened with it; where it is not,
  the choice kept before stays.
- `prompt`: optional; the words the person wrote for the agent in the form, as they wrote them. They
  are the person's, not Linear's: the prompt ends with them, after a sentence of the program's that
  says so (`The person who started you on this issue adds:`). The ends of their lines are made `\n`
  and the blank round them is taken off; blank or null is no words. Up to 4000 characters, and no
  control character but a line end or a tab (`bad-params` otherwise). They are not kept.

```json
{ "pane": "p41", "folder": "/Users/ada/acme-app", "context": "/Users/ada/.local/state/linearpen/work/20261111T031500Z-ACM-412/issues.md",
  "moved": 1, "notMoved": null }
```

`moved` is how many issues were moved (0 or 1: 0 where `progress` was not `true` or the issue had
nothing to move), and `notMoved` says why it did not (Linear refused, did not answer, or the team has
no state of work in progress), or is null. The agent is started either way: a refusal is not an error
of `start`, and the move is not sent again. The lists are changed in place, as after `change`, and
the issue keeps the mark below.

The file is `$XDG_STATE_HOME/linearpen/work/<UTC time>-<first identifier>/issues.md` (the
directories `0700`, the file `0600`); it says in its first lines that the text under it was
written by others and holds no instructions for the agent, and the branch Linear
suggests for the issue (where that is a name `git` would take). Where the description or a comment names
files (`![alt](address)`, or `[name](address)` where the address is one of Linear's files) the file has
a `Files` section before the description: how many pictures and how many other files, where, and the
command that saves them, `linearpen show <id> --files <folder>/files` with the folder the file is in
(the id only where it is a UUID; else the sentence has no id); nothing is downloaded at the start.
The text of an issue is never in the
prompt (the agent's command line): the file's path is, for `worktree` a sentence of the program's
own that asks for one, where the issue's id is a UUID, that id and the path of the program in the
sentence that says what `linearpen` can write in Linear for the agent (its status, comments, files
and new issues: `docs/cli.md`), and, last, the person's own words (`prompt`). The prompt is one word of the command line: no shell reads
it, and the person's words never begin it. The agent's command is `<agent> <prompt>` and no more, and
the `bateri` call that makes the pane carries nothing of the pen's in its environment. The id inside
the prompt is the one thing of Linear's in a command line. No flag that lets the agent do
what it would ask about is ever added. The key is not in the file.

Once the pane is made, `start` keeps the agent, the folder, the place and (where `progress` was
given) the status of the call in `$XDG_STATE_HOME/linearpen/prefs.json` (`0600`; a word that is not
a short word of lowercase letters and dashes is not written; it holds nothing of Linear's and no
key); that it cannot be kept is not an error of the call. And it puts away old runs: the folders of
`work/` older than 14 days (by the time in their names), only those that are named as above, are
real folders (not links), hold nothing but `issues.md` (a real file) and, where the agent had the
files saved, a real folder `files` of real files (at most 200), and are not the folder of a
pane this run of the program watches (a pane left from an earlier run is not known to it, and its
file goes at that age too: the agent read it long before). The file and then the folder are
removed, never a tree, and nothing outside `work/` is touched. The program does the same once as
it begins.

From here the issue of the call carries a mark in the topic (`working`) until the agent's pane ends,
and the program looks at the panes (see `working`) for as long as there is one.
Errors: `bad-params`, `no-key`, `refused`, `unreachable`, `failed` (the issue is asked of Linear
for its text), `unavailable`, `start-failed`.

### `new_options`

What a new issue can be given, and a new id for its draft. Asked when the form is opened (and again
for another team); nothing is made. Params: `{ "team": "ACM" }`, optional: a team's id or its key,
whatever the case. A team that is not named, or not there, is the first by name. Up to 64 letters,
digits and dashes.

```json
{
  "draft": "5c1c2a58-7d5e-4c0b-9b43-1c8f0f6f2d11",
  "teams": [{ "id": "team-acm", "key": "ACM", "name": "Acme" }, { "id": "team-web", "key": "WEB", "name": "Web" }],
  "team": "team-acm", "defaultState": "s-todo",
  "states": [{ "id": "s-todo", "name": "Todo", "kind": "unstarted" }],
  "allStates": [{ "id": "s-todo", "name": "Todo", "kind": "unstarted" },
                { "id": "s-done", "name": "Done", "kind": "completed" }],
  "labels": [{ "id": "l-api", "name": "api", "color": "#3e63dd" }],
  "projects": [{ "id": "p-1", "name": "Billing" }],
  "members": [{ "id": "u-ada", "name": "Ada", "me": true }],
  "cyclesEnabled": true,
  "cycles": [{ "id": "c-12", "label": "Cycle 12", "active": true },
             { "id": "c-13", "label": "Cycle 13", "active": false }],
  "estimates": [{ "value": 1, "label": "XS" }, { "value": 2, "label": "S" },
                { "value": 3, "label": "M" }, { "value": 5, "label": "L" },
                { "value": 8, "label": "XL" }]
}
```

- `draft`: the id of a draft of an issue, a UUID the program made. It is a new one each time; the
  page keeps the first it was given until its issue is made, and sends it as `create`'s `id`.
- `teams`: the teams an issue can be made in, by name (at most 50). `team`: the id of the team the
  rest is of.
- `states`: the open states of the team: `triage`, `backlog`, `unstarted`, `started`, in that order
  and by position within a kind (Done and Canceled are not offered): what a new issue begins in.
  `defaultState` is the team's own state for a new issue, else the first `unstarted`, else the first
  `backlog`; it is one of `states` or null.
- `allStates`: every state an issue of the team can be put in (what `change` takes as `status`): the
  same, then `completed` and `canceled`, in the order of the kinds and by position within one. A
  state that is a duplicate (its own kind where Linear has one, or one of the canceled kind that is
  named `Duplicate`) is not in it: it wants the issue it duplicates, which a change does not name.
- `labels`: the team's labels and the workspace's, once each, by name; a label that is a group of
  labels is not offered. `color` is Linear's own (`#rrggbb`). At most 100.
- `projects`: the team's projects that are not finished or canceled, by name (at most 50).
- `members`: the team's people who are active, by name (at most 100); `me` marks the person whose
  key it is.
- `cyclesEnabled`: whether the team has cycles. `cycles`: what an issue of it can be put in (what
  `change` takes as `cycle`): the cycle that runs (`active`), then the next ones by number, at most
  six (Linear is asked for the first fifty upcoming ones, newest first, and they are sorted here);
  `label` is the cycle's name where it has one, else `Cycle 12`. It is empty where the team has no
  cycles.
- `estimates`: the scale the team estimates by, smallest first (what `change` takes as `estimate`):
  `exponential` 1 2 4 8 16 (and 32 64 where the team has the extended scale), `fibonacci` 1 2 3 5 8
  (13 21), `linear` 1 2 3 4 5 (6 7), `tShirt` XS S M L XL (XXL XXXL), whose `value`s are the Fibonacci
  numbers Linear takes them as (1 2 3 5 8, 13 21), each with its `label`; a `0` comes first where the
  team allows an estimate of 0. It is empty where the team does not estimate.

The teams and what each offers are asked of Linear when first needed and kept for the run; they are
let go whenever the program asks for the lists again (`refresh`, and every five minutes). Errors:
`bad-params`, `no-key`, `refused`, `unreachable`, `failed` (also where Linear has no team, and where
the program could not make the id of the draft).

### `create`

Makes the issue in Linear. Params:

```json
{ "id": "5c1c2a58-7d5e-4c0b-9b43-1c8f0f6f2d11", "team": "team-acm", "title": "Fix the login",
  "description": "It fails on the second try.", "state": "s-todo", "priority": "high",
  "assignee": "u-ada", "labels": ["l-api"], "project": "p-1" }
```

- `id`: the id of the draft (`new_options`' `draft`), a UUID. Linear takes it as the id of the issue.
- `team`: the id of the team. `title`: one to 255 characters once white space at both ends is gone.
- Optional: `description` (markdown, at most 50 000 characters; white space alone is none), `state`,
  `assignee`, `project` (ids from `new_options`), `labels` (at most 50 ids), `priority` (`none`,
  `urgent`, `high`, `medium` or `low`; sent to Linear as 0 to 4). What is left out, or empty, is not
  sent.
- `files`: as for `comment`'s, at most ten; they are put in Linear's storage first and their lines
  come at the end of the description (the files alone are a description). Where one cannot be sent
  no issue is made (`upload-failed`).

```json
{ "id": "5c1c2a58-7d5e-4c0b-9b43-1c8f0f6f2d11", "identifier": "ACM-413" }
```

The issue is sent **once**. If Linear's answer did not come whole (nothing came, or an error), the
program does not send it again: it asks Linear for the issue by the draft's id, and if it is there
the issue was made and that is the answer. If it is not, the call fails; the person may ask again,
with the same `id`, which cannot make a second issue. A key Linear does not take (`refused`) or one
that may not create issues is not asked about.

The program asks for both lists again when the issue is made, and the topic `issues` tells them when
they come: the answer does not wait for them, and the page looks for the row in what the topic tells.

Errors: `bad-params` (also an `id` that is not a UUID), `no-key`, `refused`, `unreachable`, and
`create-failed`: Linear did not make it. The words say why: `Linear did not make it: this key cannot
create issues` where the key is one that reads and may not write, else Linear's own words
(`Linear did not make it: ...`). Where the answer was lost and asking for the issue by the id did
not settle it either, it is not known that Linear did not make it, and the words say so
(`Linear did not answer whole, so it may have made the issue: press Create again to find out`);
pressing Create again sends the same `id`, which cannot make a second issue.

### `change`

Changes one field of one issue or of several, in Linear. Params:

```json
{ "ids": ["issue-412", "issue-405"], "status": "s-done" }
```

- `ids`: Linear's ids of the issues (a row's `id`), from one to 50 (Linear's own limit of a batch);
  up to 64 letters, digits, dashes and underscores each. One said twice is one.
- **Exactly one** of these, which is the field that is changed (two, or none, is `bad-params`; so is
  any member this call does not take, `description` and `team` among them: neither is written):
  `status` (the id of a state, from `new_options`' `allStates`), `priority` (`none`, `urgent`,
  `high`, `medium` or `low`), `assignee` (the id of a person, or `null` for no one), `addLabel` and
  `removeLabel` (the id of one label the issues get or lose), `project` (the id of a project, or
  `null` for none), `cycle` (the id of a cycle, from `new_options`' `cycles`, or `null` for none),
  `estimate` (a whole number of points from 0 to 1 000, as `new_options`' `estimates` say, or `null`
  for none), `dueDate` (a day written `2026-11-15` that is on the calendar, or `null` for none),
  `title` (one to 255 characters; it is made one line, with no white space at its ends).

```json
{ "issues": [{ "id": "issue-412", "listed": false }, { "id": "issue-405", "listed": true }] }
```

The answer tells the issues Linear changed, and for each whether the list in view (the list the
page last set with `view`) still holds it: one that is done or canceled is in no list, and one given
to another person is not in My issues. What Linear says of them is written into the lists the program holds (My issues and the one
shown: a list other than All issues keeps an issue it holds and lets it go where it is done, a search,
which holds closed issues too, keeps it) and into
the issues kept for `issue`, and the topic `issues` tells it: the lists are not asked for again.

The change is sent **once**, to all the issues in one request; the program never sends it again by
itself, and asking twice for the same value changes nothing more. If no answer came,
the call fails, and the program asks Linear for each of the issues, so that what the topic tells is
what Linear holds; the call fails all the same. An answer that cannot be read (an HTTP status that is
not 200 with no error in it, such as a gateway's 502, or a body that is not what was asked for) is
the same as no answer: the issues are asked for, and the call fails with `change-failed` and the
words that it is not known whether Linear made it. An answer that says no (Linear's own error, or
`refused`: the key is not taken) says the change was not made, and nothing is asked about. A list
that was on its way when the change was made, or when the issues were asked for, was asked for
before it: when it has come it is let go, and the lists are asked for once more, so that the old
values are not put back.

Errors: `bad-params`, `no-key`, `refused`, `unreachable` (no answer came: the change may have been
made, and the topic tells what Linear holds), and `change-failed`: Linear did not make it, or it is
not known. The words say why: `Linear did not change it: this key cannot change issues` where the key
reads and may not write, `Linear did not answer whole, so it may have changed it` where the answer
could not be read, else Linear's own words (`Linear did not change it: ...`).

### `comment`

Posts a comment on an issue, in Linear, in the words the person wrote. Params:

```json
{ "issue": "issue-412", "body": "Looks right to me",
  "files": [{ "handle": "0123456789abcdef0123456789abcdef", "name": "shot.png" }] }
```

- `issue`: Linear's id of the issue (the `id` of a row or of `issue`), or its identifier; up to 64
  letters, digits, dashes and underscores.
- `body`: the words, as they were written (Markdown, which is what Linear takes), from one to 10 000
  characters, with something in them besides white space (or nothing, where there are `files`).
  They are not changed, and they are a variable of the request and never part of its text.
- `files`: the files that go with it, at most ten, each the `handle` the terminal made of a file the
  person gave the page (`files` event) and its `name`. Each is of any kind and at most 100 MiB: it is
  received from the terminal a piece at a time into a copy of the program's own (never held whole in
  memory), told to be what its first bytes say it is (`image/png`, `image/jpeg`, `image/gif`,
  `image/webp`, `application/pdf`; `application/octet-stream` for the rest) and put in Linear's storage
  before the comment is posted; the comment is the words, a blank line and one line for each: `![name](address)`
  for a picture (PNG, JPEG, GIF, WebP) and `[name](address)` for any other file. Where one cannot be
  sent, or has nothing in it, nothing is posted (`upload-failed`, which says which).

  Nothing else is taken (`bad-params`): not a comment to reply under, not a name to write as, not a
  path to a file (the handle is the only way to one). The program makes the id the comment is posted
  under (a UUID version 4, as for `create`'s drafts).

```json
{ "comment": { "author": "Ada", "at": "Nov 12", "body": "Looks right to me" } }
```

The answer is the comment as Linear says it is, in the form the comments of `issue` have. It is put
among the comments of the issue kept for `issue`, so that the page asks for the issue again and has
it.

The comment is sent **once**; the program never sends it again by itself. If no answer came, or what
came cannot be read (an HTTP status that is not 200 with no error in it, such as a gateway's 502),
the call fails and the program asks Linear for the issue, so that what `issue` tells is what Linear
holds; the words say that it may have been posted, and the person looks at the comments before they
write it again. An answer that says no (Linear's own error, or `refused`: the key is not taken) says
it was not posted, and nothing is asked about.

Errors: `bad-params`, `no-key`, `refused`, `failed` (the program could not make an id for the
comment, and sent nothing), and `comment-failed`: Linear did not post it, or it is not known. The words say why: `Linear did not post it: this key cannot comment` where the key reads and
may not write, `Linear did not post it` where Linear answered without posting it, `Linear did not
answer whole, so it may have posted it: look at the comments before writing it again` where the
answer could not be read, `Linear could not be reached: ...; it may have been posted: look at the
comments before writing it again` where no answer came, else Linear's own words (`Linear did not
post it: ...`).

### `picture`

A file of Linear in the markdown of an issue, given to the terminal so that the page can draw it: a
PNG, a JPEG or a PDF of at most 8 MiB. The program fetches it with the key, from
`https://uploads.linear.app/` and from nowhere else, and holds it for `owner` (an issue). Params:
`{ "src": "https://uploads.linear.app/...", "owner": "issue-412" }`; `owner` is the issue's id.

```json
{ "name": "sha256:3b4a...", "width": 1280, "height": 720 }
```

`name` is what the page draws it by (`image(name)`), `width` and `height` are in pixels. For a PDF the
answer has `"pdf": true`, `"pages"` (how many, where the file says; left out where it does not: the PDF
is then drawn page by page with no end known) and `width` and `height` are the first page's, in points
(a Letter page where the file does not say); the page draws a page with `image(name):page(n)`. A file
that is not drawn is an answer, not an error: `{ "refused": "format" }` (not a PNG, a JPEG or a PDF:
a GIF, a WebP, a video, a log, a zip), `"too-large"` (over 8 MiB, or a picture over 4096 pixels on a
side), `"not-linear"` (the address is not one of Linear's files: it is not fetched, with the key or
without), `"unreachable"` (Linear did not give it) or `"no-view"` (the terminal has no view that holds
files: the page draws the words of the file as it would without them). The page draws a card for each
of them, with one verb (Open for a PDF, `save` for any other); the rest is its menu. Where the bytes say what the file
is, the answer carries `"kind"` (`"png"`, `"jpg"`, `"gif"`, `"webp"`), and where a picture is over 4096
pixels on a side, `"width"` and `"height"` too, so that the card can say why it is not drawn ("GIF is
not shown here", "5120×2880  Too large to show here"); a file whose bytes say nothing has neither.
A file asked for again, by that
address, for another owner is not fetched again. At most four are fetched at once; what is held is at
most 64 MiB, the oldest let go first. Errors: `bad-params`, `no-key`.

### `release`

Lets go of what is held for `owner`: the files that no other owner has asked for are let go of by
the terminal. The page calls it when the issue is no longer open. Params: `{ "owner": "issue-412" }`.
Answers `{}`.

### `save`

A file of Linear in the markdown of an issue, saved where the person chose. The page asks the
terminal for a place (the reply `save`, to the person's click or key, with the file's name) and the
terminal tells the page its handle (`saving` event); the page then calls `save`. The program fetches
the file with the key, from `https://uploads.linear.app/` and from nowhere else, at most 100 MiB, into a
copy of its own that nothing else can read (never into memory), writes it to the place in pieces of
1 MiB, and takes the copy away. Params: `{ "src": "https://uploads.linear.app/...", "handle": "0123..." }`.
Answers `{ "size": 1048576 }`, in bytes. The saving that was not asked for (a handle that is not the
person's) is the terminal's to refuse. Errors: `bad-params`, `no-key`, `refused`, and `save-failed`
with words that say why (`the file is not one of Linear's`, `the file is over 100 MiB`, `Linear did not
give the file`, `the place did not take the file`, `this terminal cannot save files`) and no address.

### `pdf-open`

A PDF the page shows large, read by the program for what the terminal does not say: how many pages it
has and how large each is, and where the words of a page and every letter of them stand, to select and
copy them and to find words in it. The terminal draws the pages (`image(name):page(n)`); the program
reads the same file (fetched again with the key, from `https://uploads.linear.app/` and from nowhere
else, at most 8 MiB) and holds it for `owner` until `pdf-close`, `release` of that owner, or another
PDF is opened: at most one PDF is held. Params: `{ "src": "https://uploads.linear.app/...", "owner":
"issue-412" }`.

```json
{ "pages": 12, "sizes": [[11, 612, 792], [1, 792, 612]], "text": true }
```

`sizes` are runs of pages of one size, in order: how many, the width and the height in points, as the
terminal draws a page (the page's own size, `Rotate` applied; a page is never taken for more than 16 384
points on a side). The sum of the first members is `pages`. `text` is whether any page has text (a PDF
of pictures has none, and finding in it finds nothing); looking for it is cut short after a second, and
a PDF it did not get through is taken to have some. A PDF that cannot be read (broken, encrypted, with
no page) or has more than 10 000 pages is an error, `pdf-failed`, with words that say which; the page
then shows the pages as the terminal draws them, with nothing to select. Errors: `bad-params`,
`no-key`, `refused`, `pdf-failed`.

### `pdf-close`

Lets go of the PDF held for `owner`. Params: `{ "owner": "issue-412" }`. Answers `{}`.

### `pdf-text`

The words of one page of the PDF held for `owner`: its lines from the top down and from the left, each
with its box and the place of every letter, in points from the page's top left corner as the page is
shown. Params: `{ "owner": "issue-412", "page": 3 }` (from 1).

```json
{ "page": 3, "lines": [{ "x": 72, "y": 90.5, "w": 210.25, "h": 13.4, "text": "Rate limits", "xs": [0, 6.7, 12.1, 18.3, 23, 28.4, 30.1, 36.6, 42, 47.5, 53, 59.2] }], "truncated": false }
```

A line is the letters of one baseline, in the order of their places and not of the file's, with a space
where the ink leaves the gap a word has; a gap of more than three letter sizes breaks it in two (a
column, a cell of a table). `xs` are the places across the line of each character's left edge from `x`,
and then the right edge of the last: one more than the line has characters. Text that is not set along
the page (turned a quarter, upside down) is left out, and so is a letter the font says no character of;
a ligature is its letters. A page with no text, or whose content cannot be read, has no lines. A line
is at most 4096 characters (a row of more is several lines), a page is read for at most 200000 letters
and 5 seconds (what was read by then is the page's text), and a message holds at most a quarter of
a MiB of lines, and one line at least. `truncated` is true where lines were left out that did not fit one
message. Errors: `bad-params`,
`pdf-failed` (`That PDF is not open`, `This PDF has no page 3`).

### `pdf-find`

Where words are in the text of every page of the PDF held for `owner`, in the order of the pages. A
match is within a line. Params: `{ "owner": "issue-412", "query": "rate limit", "caseSensitive": false }`;
`query` is at most 200 characters and found as it is written (no pattern), case folded one character at
a time unless `caseSensitive` is true.

```json
{ "matches": [{ "page": 3, "x": 72, "y": 90.5, "w": 61.5, "h": 13.4 }], "searched": 12, "truncated": false, "ms": 41 }
```

`searched` is how many pages were read, `ms` how long it took. The search stops at 2 000 matches, when
the message would not hold more, after 30 seconds, or when the page gives the call up (a `cancel`), and
says `truncated`. It reads the pages for the search and does not keep them: a search through a book does
not push out the page that is being read. Errors: `bad-params`, `pdf-failed` (`That PDF is not open`).

### `describe`

Adds files at the end of the description of an issue: the one way the pen writes a description.
Params: `{ "id": "issue-412", "files": [{ "handle": "...", "name": "shot.png" }] }`, from one to ten
(as for `comment`'s `files`). The files are put in Linear's storage first; then the program reads
the description **whole** from Linear (what the page holds of it is never used, and it is cut for the
page), adds a blank line and the lines of the files, and sends that, and nothing else, as the
description of that issue. A description that cannot be read is not written. It is sent once; the
program never sends it again by itself. Answers `{}` and the issue kept for `issue` is the one Linear
holds now, which the page asks for again.

Errors: `bad-params`, `no-key`, `refused`, `upload-failed` (nothing was written), and `describe-failed`:
`the description could not be read, so nothing was written (...)` and `the description would be too
long with the files, so nothing was written` where nothing was sent (the files are in Linear's storage
all the same, and are sent again at the next try), `Linear did not change it: ...` where it did not write it, `Linear did not answer whole, so it may have
added them: look at the description before adding them again` where it is not known, and the same
look-before-again words where no answer came after it was sent.

### `refresh`

Asks Linear for My issues and the list shown again, for the names of the lists the page can go to, and
for the teams and choices of a new issue the next time they are needed. It answers at once with `{}`; the topic tells the lists when they come. Params: none. Errors:
`no-key`.

## Topic

### `issues`

The list, told when it is subscribed to and whenever the program has something new (asking began,
the lists came, asking failed). The same value `view` answers with.

```json
{
  "loaded": true, "refreshing": false, "error": null,
  "scope": "mine", "listId": null, "listing": "ready", "filter": "", "matched": 12,
  "filters": { "status": [], "priority": [], "assignee": [], "labels": [], "project": [] },
  "counts": { "mine": 14, "mineCut": false, "shown": 14, "shownCut": false },
  "catalog": {
    "loaded": true, "error": null,
    "items": [
      { "key": "view:view-bugs", "kind": "view", "id": "view-bugs", "title": "Bugs to triage",
        "section": "favorite", "team": null, "cycle": null },
      { "key": "cycle:c-24", "kind": "cycle", "id": "c-24", "title": "ACM \u00b7 Current cycle",
        "section": "team", "team": "ACM", "cycle": { "number": 24, "endsAt": "2026-11-15T00:00:00.000Z", "ends": "Nov 15" } }
    ]
  },
  "groups": [
    { "key": "started:In Progress", "name": "In Progress", "kind": "started",
      "count": 4, "closed": false,
      "rows": [
        { "id": "issue-412", "identifier": "ACM-412", "title": "Add rate limiting",
          "url": "https://linear.app/acme/issue/ACM-412/add-rate-limiting",
          "branchName": "ada/acm-412-add-rate-limiting",
          "priority": "high", "updated": "Nov 9",
          "labels": [{ "id": "l-api", "name": "api", "color": "#3e63dd" }], "assignee": null,
          "teamId": "team-acm", "stateId": "s-prog", "assigneeId": "u-ada", "projectId": null,
          "cycleId": "c-12", "estimate": null, "dueDate": null }
      ] }
  ],
  "updated": 1762870000, "organization": "Acme", "person": "Ada",
  "working": { "issue-412": { "agent": "Claude", "idle": false } }
}
```

- `loaded` is false until the first answer of Linear comes; `error` then is why it has not come
  (`{ "kind", "message" }`, the kinds of the errors above). With `loaded` the lists are the last that
  came, and an `error` says that the last asking failed.
- `refreshing`: Linear is being asked now.
- `scope` is the kind of the list shown (`mine`, `all`, `team`, `cycle`, `view`, `project`, `label` or
  `search`) and `listId` its id (null for `mine` and `all`; the words, for a `search`).
- `filters`: what the lists are narrowed by in Linear, as `view` set it.
- `listing`: whether the list shown has come: `ready`; `loading` (it is being asked for: `groups` is
  empty, and `counts.shown` is 0); or `failed` (Linear could not tell it: `error` says why, and
  `loaded` is still true because My issues came).
- `counts`: how many open issues My issues holds (`mine`, whole) and the list shown holds (`shown`, as
  `filters` narrow it, a search's closed issues too; not by the words of `filter`); `mineCut` and `shownCut` say there are more than the 500 the program takes of a list:
  the 500 that changed last, and for a search the first 500 Linear finds (it has no order of its own).
- A row tells at most 10 labels of its issue (the first Linear gives); the 11th is not marked.
- `catalog`: the lists the page can go to besides My issues and All issues, as Linear last said them.
  `loaded` is false until the first answer, and `error` says why one could not come (the lists of
  before stay). Each `item` has `key` (`kind:id`, what the page keeps it by), `kind` and `id` (what
  `view`'s `list` takes), `title` (named as the card shows it: a team by its key, `ACM \u00b7 Active
  issues` and, where it has a cycle running, `ACM \u00b7 Current cycle`), `section` (`favorite`, in
  the order Linear's sidebar has them; `team`, the teams the person is a member of in the order of
  their own list of teams; `view` and `project`, by name), `team` (the
  key of the team a view is of, or null) and, for a `cycle`, its `number`, when it `endsAt` and the day it
  `ends` as the dates of the lists are written (`Nov 15`). A
  favorite that is a folder, or a view Linear defines itself, is not an item. At most 200 of each
  kind are asked for.
- `matched`: how many issues of the list shown the filter lets through.
- `groups`: by the kind of the state (`started`, `unstarted`, `backlog`, `triage`, then
  `completed`, `canceled`, `other`), within a kind by the position of the state; states of one name
  in several teams are one group. A group is named by `key`, `kind:name`. A folded group has `count`
  and `closed: true` and no `rows`. In a group the issue updated last is first.
- `url`: where the issue is in Linear, and `branchName`: the name of the branch Linear suggests for it
  (null where it has none): what the page copies (`Copy link`, `Copy branch name`). Neither holds a
  control character.
- `priority`: `none`, `urgent`, `high`, `medium` or `low`. `updated`: the day of the last change as
  the list writes it. `assignee` is set in every list but My issues: `"you"`, a name, or null.
- A label's `color` is Linear's own (`#rrggbb`): data, not a role of the theme.
- `teamId`, `stateId`, `assigneeId` (null where unassigned), `projectId` (null where in none) and
  `cycleId` (null where in no cycle) are Linear's ids of what the issue is of and in, and each label
  has its `id`: a menu marks what the issue has by them, and a change names them (`change`).
  `estimate` is its points (a whole number, null where it has none) and `dueDate` the day it is due
  (`2026-11-15`, or null). Whether several issues are of one team is told by their `teamId`s.
- `updated`: when the lists came, seconds since the epoch; `organization` and `person` are the
  workspace's name and the person's. Both are null before the first answer.
- `working`: the issues an agent was started on and whose pane has not ended, by Linear's id, with
  the agent's name. It is not kept between runs. `idle` is true where the agent has stopped: it is
  done, or it waits for the person (the two cannot be told apart). The program asks bateri
  (`bateri ls --json`) every three seconds for as long as a pane is watched (and not at all when
  none is), and an agent has stopped when its pane's `output_at` is more than ten seconds old. Only
  the `id` and `output_at` of each terminal are read. Where the list cannot be read, has no such
  terminal, or the terminal has no time that is a number, `idle` is false: the look only adds the
  word and never takes the mark away, which goes when the pane ends. `idle` goes back to false when
  the pane writes again.
