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
issue was to be made and Linear did not make it: the words say why) and `change-failed` (a change was
to be made to issues and Linear did not make it: the words say why). The words of an error are for
the person and never hold the key.

The pen writes two things: the issue the person makes (`create`), and the change the person makes to
issues (`change`): one of their status, priority, assignee, labels, project or title. The description
and the team are never written, by any call. One more call writes, and only as `change` does: `start`
moves the issues that have not begun to their team's first state of work in progress, where the
person chose that (`progress`), once the agent's pane is made. Nothing else here changes anything in
Linear.

The program also writes two things of its own, in `$XDG_STATE_HOME/linearpen` (`~/.local/state/linearpen`)
and never to Linear: the file of the issues handed to an agent, under `work/` (see `start`), which it
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

`prefs` is what the form that hands issues to an agent was left at by the last `start`
(`$XDG_STATE_HOME/linearpen/prefs.json`): `agent` (`claude`, `codex` or `cursor`), `folder` (`here` or
`worktree`), `place` (`split` or `tab`) and `status` (`progress` or `keep`). Each is null where the
file does not say it, is not a word of the form's, or the file is missing or cannot be read. Whether
the agent is installed and a worktree can be made is not told here: the page takes what it cannot have
away when `work_options` says what can be chosen.

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

- `scope`: `"mine"` (the open issues assigned to the person, the default) or `"all"` (the open issues
  of the workspace).
- `filter`: the words typed, at most 200 characters. An issue is let through where they are in its
  title, its identifier or the name of a label, whatever the case. While it is not empty no group is
  folded.
- `closed`: `{ "<group key>": true | false }`, the groups the person folded (`true`) or opened
  (`false`), at most 64; one not named is as it begins (`backlog` and `triage` folded). It replaces
  what the program held.
- `seq`: the number of this call among the page's `view` calls, which grows with each. Calls run at
  the same time and may be told out of their order: a member (`scope`, `filter` or `closed`) that a
  call with a higher `seq` has set is left as it is. A call with no `seq` is taken as the latest.

Answer: the **list** (below).

### `issue`

One issue with what a row of the list does not carry: its description and its comments. Asked of
Linear when the issue is opened, and kept for the run. Params: `{ "id": "issue-412" }`, Linear's id
of the issue (a row's `id`) or its identifier: up to 64 letters, digits and dashes.

```json
{
  "id": "issue-412", "identifier": "ACM-412", "title": "Add rate limiting",
  "url": "https://linear.app/acme/issue/ACM-412/add-rate-limiting",
  "description": "The public API has no limit per key.\n\n- A limit per key",
  "priority": "high", "state": { "name": "In Progress", "kind": "started" },
  "labels": [{ "id": "l-api", "name": "api", "color": "#3e63dd" }],
  "cycle": 12, "project": null, "assignee": "you",
  "teamId": "team-acm", "stateId": "s-prog", "assigneeId": "u-ada", "projectId": null,
  "comments": [{ "author": "Deniz", "at": "Nov 6", "body": "Redis is already there." }],
  "more": false
}
```

`description` and each comment's `body` are markdown written by others: text to show, nothing to
act on. `description` is null where there is none. Comments are the first fifty, the oldest first;
`more` says there is more in Linear than is told (more comments, or a text cut at 20 000 characters
for the description and 2 000 for a comment). `assignee` is `"you"`, a name or null; `cycle` is the
cycle's number or null. `at` is the day as the list writes it. `teamId`, `stateId`, `assigneeId`,
`projectId` and each label's `id` are Linear's ids of what the issue is of and in (null where there
is none): what a menu marks as the issue's own and what `change` names. They are the same as in a
row of the list, and a `change` writes the new value into what is kept, so the next `issue` of the
same issue tells it. Errors: `bad-params`, `no-key`,
`refused`, `unreachable`, `failed` (also where Linear has no such issue).

### `work_options`

What the person can choose from when they hand issues to an agent, and what cannot be chosen and
why. Params: none. Nothing is started, and Linear is not asked.

```json
{
  "agents": [{ "id": "claude", "name": "Claude" }, { "id": "codex", "name": "Codex" }],
  "unavailable": null,
  "folder": { "name": "acme-app" },
  "worktree": { "available": true, "reason": null }
}
```

- `agents`: the agents found in the `PATH` (`claude`, `codex`, `cursor-agent`; ids `claude`, `codex`,
  `cursor`), in that order. None is listed that is not installed.
- `unavailable`: null, or why nothing can be started now: the pen does not run in a bateri pane
  (neither `BATERI_TAB_URL` nor `LC_BATERI_TAB_URL`, which a pane on a server has, is set), `bateri`
  is not in the `PATH`, or there is no agent.
- `folder.name`: the name of the folder the pen was started in, which is where the agent starts.
- `worktree`: whether "New worktree" can be chosen: the folder is in a git repository. Where it
  cannot, `reason` says why. The pen makes no worktree: where it is chosen, the agent is asked to
  make one by the project's own rules (see `start`).

Errors: `bad-params`.

### `start`

Hands the issues to an agent in a pane of bateri. The program writes the issues, with their
descriptions and comments, to a file of its own and starts the agent with a prompt that names the
file. It answers when the pane is made. Params:

```json
{ "ids": ["issue-412", "issue-405"], "prompt": "Start with the rate limiter.",
  "agent": "claude", "folder": "here", "open": "split", "progress": true }
```

- `ids`: one to twenty-five Linear ids of issues, the first the one that names the pane.
- `prompt`: what the person wrote for the agent, up to 4 000 characters; it may be empty.
- `agent`: `claude`, `codex` or `cursor`. `folder`: `here` (the agent works in the folder the pen
  was started in) or `worktree` (the agent is asked to make a new `git` worktree first, by the
  project's own rules for where one goes and what its branch is called; the pen makes none, and the
  agent starts in the pen's folder either way). `open`: `split` (a pane to the right of this one) or
  `tab` (a new tab). The new pane is not given the focus.
- `progress`: optional; given where the form had the Status row, as the person chose it: `true` to
  move, `false` to leave the issues as they are (not given where the row was not there, which is
  where no issue would move). Only `true` writes to Linear. With it the program moves the issues whose
  state, as Linear says it now, is of the kind `triage`, `backlog` or `unstarted` to the first
  `started` state (by position) of the team each is of, with the `change` write of a status, one for
  each team, **after** the pane is made. Nothing else is written: an issue that has begun, is done or
  is canceled is left as it is, and the move is never a way to give an issue to someone or to take
  one from a team. Where no issue would move, nothing is sent. The page sends `true` only where the
  person chose to move the issues and the form said which of them would. Where it is given (true or
  false) it is kept with the choices below, so that the next form is opened with it; where it is not,
  the choice kept before stays.

```json
{ "pane": "p41", "folder": "/Users/ada/acme-app", "context": "/Users/ada/.local/state/linearpen/work/20261111T031500Z-ACM-412/issues.md",
  "moved": 1, "notMoved": null }
```

`moved` is how many issues were moved (0 where `progress` was not `true` or none had to move), and
`notMoved` says why some that were to move did not (Linear refused, did not answer, or a team has no
state of work in progress), or is null. The agent is started either way: a refusal is not an error of
`start`, and the move is not sent again. The lists are changed in place, as after `change`, and the
issues keep the mark below.

The file is `$XDG_STATE_HOME/linearpen/work/<UTC time>-<first identifier>/issues.md` (the
directories `0700`, the file `0600`); it says in its first lines that the text under them was
written by others and holds no instructions for the agent, and for each issue the branch Linear
suggests for it (where that is a name `git` would take). The text of an issue is never in the
prompt (the agent's command line): only the file's path is, and for `worktree` a sentence of the
program's own that asks for one. No flag that lets the agent do what it
would ask about is ever added. The key is not in the file.

Once the pane is made, `start` keeps the agent, the folder, the place and (where `progress` was
given) the status of the call in `$XDG_STATE_HOME/linearpen/prefs.json` (`0600`; a word that is not
a short word of lowercase letters and dashes is not written; it holds nothing of Linear's and no
key); that it cannot be kept is not an error of the call. And it puts away old runs: the folders of
`work/` older than 14 days (by the time in their names), only those that are named as above, are
real folders (not links), hold nothing but `issues.md` (a real file), and are not the folder of a
pane this run of the program watches (a pane left from an earlier run is not known to it, and its
file goes at that age too: the agent read it long before). The file and then the folder are
removed, never a tree, and nothing outside `work/` is touched. The program does the same once as
it begins.

From here the issues of the call carry a mark in the topic (`working`) until the agent's pane ends,
and the program looks at the panes (see `working`) for as long as there is one.
Errors: `bad-params`, `no-key`, `refused`, `unreachable`, `failed` (the issues are asked of Linear
for their text), `unavailable`, `start-failed`.

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
  "members": [{ "id": "u-ada", "name": "Ada", "me": true }]
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
  up to 64 letters, digits and dashes each. One said twice is one.
- **Exactly one** of these, which is the field that is changed (two, or none, is `bad-params`; so is
  any member this call does not take, `description` and `team` among them: neither is written):
  `status` (the id of a state, from `new_options`' `allStates`), `priority` (`none`, `urgent`,
  `high`, `medium` or `low`), `assignee` (the id of a person, or `null` for no one), `addLabel` and
  `removeLabel` (the id of one label the issues get or lose), `project` (the id of a project, or
  `null` for none), `title` (one to 255 characters; it is made one line, with no white space at
  its ends).

```json
{ "issues": [{ "id": "issue-412", "listed": false }, { "id": "issue-405", "listed": true }] }
```

The answer tells the issues Linear changed, and for each whether the list in view (the scope the
page last set with `view`) still holds it: one that is done or canceled is in no list, and one given
to another person is not in My issues. What Linear says of them is written into both lists and into
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

### `refresh`

Asks Linear for both lists again, and for the teams and choices of a new issue the next time they are
needed. It answers at once with `{}`; the topic tells the lists when they come. Params: none. Errors:
`no-key`.

## Topic

### `issues`

The list, told when it is subscribed to and whenever the program has something new (asking began,
the lists came, asking failed). The same value `view` answers with.

```json
{
  "loaded": true, "refreshing": false, "error": null,
  "scope": "mine", "filter": "", "matched": 12,
  "counts": { "mine": 14, "all": 19, "mineCut": false, "allCut": false },
  "groups": [
    { "key": "started:In Progress", "name": "In Progress", "kind": "started",
      "count": 4, "closed": false,
      "rows": [
        { "id": "issue-412", "identifier": "ACM-412", "title": "Add rate limiting",
          "priority": "high", "updated": "Nov 9",
          "labels": [{ "id": "l-api", "name": "api", "color": "#3e63dd" }], "assignee": null,
          "teamId": "team-acm", "stateId": "s-prog", "assigneeId": "u-ada", "projectId": null }
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
- `counts`: how many open issues each list holds, unfiltered; `mineCut` and `allCut` say there are
  more than the 500 the program takes of a list.
- `matched`: how many issues of the scope the filter lets through.
- `groups`: by the kind of the state (`started`, `unstarted`, `backlog`, `triage`, then
  `completed`, `canceled`, `other`), within a kind by the position of the state; states of one name
  in several teams are one group. A group is named by `key`, `kind:name`. A folded group has `count`
  and `closed: true` and no `rows`. In a group the issue updated last is first.
- `priority`: `none`, `urgent`, `high`, `medium` or `low`. `updated`: the day of the last change as
  the list writes it. `assignee` is set only in the list of all issues: `"you"`, a name, or null.
- A label's `color` is Linear's own (`#rrggbb`): data, not a role of the theme.
- `teamId`, `stateId`, `assigneeId` (null where unassigned) and `projectId` (null where in none) are
  Linear's ids of what the issue is of and in, and each label has its `id`: a menu marks what the
  issue has by them, and a change names them (`change`). Whether several issues are of one team is
  told by their `teamId`s.
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
