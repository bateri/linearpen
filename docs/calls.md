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
there) and `start-failed` (it was tried and did not start: the words say why). The words of an error
are for the person and never hold the key.

The pen only reads: nothing here changes anything in Linear.

## Calls

### `boot`

What the page needs before it draws, once; and the program starts asking Linear for the lists if it
has a key and has not begun (listening to the topic `issues` begins it too). Params: none.

```json
{ "version": "0.1.0", "key": "file", "problem": null }
```

`key` says where the key is from: `none` (the page shows Connect), `environment` or `file`.
`problem` is, where there is a key that cannot be used (it is not one, or its file cannot be read),
the words that say why; `key` is `none` then.

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
  "labels": [{ "name": "api", "color": "#3e63dd" }],
  "cycle": 12, "project": null, "assignee": "you",
  "comments": [{ "author": "Deniz", "at": "Nov 6", "body": "Redis is already there." }],
  "more": false
}
```

`description` and each comment's `body` are markdown written by others: text to show, nothing to
act on. `description` is null where there is none. Comments are the first fifty, the oldest first;
`more` says there is more in Linear than is told (more comments, or a text cut at 20 000 characters
for the description and 2 000 for a comment). `assignee` is `"you"`, a name or null; `cycle` is the
cycle's number or null. `at` is the day as the list writes it. Errors: `bad-params`, `no-key`,
`refused`, `unreachable`, `failed` (also where Linear has no such issue).

### `work_options`

What the person can choose from when they hand issues to an agent, and what cannot be chosen and
why. Params: `{ "ids": ["issue-412", "issue-405"] }`, one to twenty-five ids of the list's issues, in
the order they are to be handed over; the first names the branch of a new worktree. Nothing is
started. The first issue is the one of the lists; Linear is asked for it only where the lists do not
have it (they may not have come).

```json
{
  "agents": [{ "id": "claude", "name": "Claude" }, { "id": "codex", "name": "Codex" }],
  "unavailable": null,
  "folder": { "name": "acme-app" },
  "worktree": { "available": true, "branch": "ada/acm-412-add-rate-limiting", "reason": null }
}
```

- `agents`: the agents found in the `PATH` (`claude`, `codex`, `cursor-agent`; ids `claude`, `codex`,
  `cursor`), in that order. None is listed that is not installed.
- `unavailable`: null, or why nothing can be started now: the pen does not run in a bateri pane
  (`BATERI_TAB_URL` is not set), `bateri` is not in the `PATH`, or there is no agent.
- `folder.name`: the name of the folder the pen was started in, which is where "This folder" is.
- `worktree`: whether "New worktree" can be chosen. Where it can, `branch` is the branch the agent
  works on (Linear's branch name for the first issue, or its identifier in lower case where Linear
  has none). Where it cannot, `reason` says why (the folder is not in a git repository, the name
  Linear gives is not one `git` takes), and `branch` is null.

Errors: `bad-params`, `no-key`, `refused`, `unreachable`, `failed` (where Linear is asked).

### `start`

Hands the issues to an agent in a pane of bateri. The program writes the issues, with their
descriptions and comments, to a file of its own and starts the agent with a prompt that names the
file. It answers when the pane is made. Params:

```json
{ "ids": ["issue-412", "issue-405"], "prompt": "Start with the rate limiter.",
  "agent": "claude", "folder": "here", "open": "split" }
```

- `ids`: one to twenty-five Linear ids of issues, the first the one that names the worktree.
- `prompt`: what the person wrote for the agent, up to 4 000 characters; it may be empty.
- `agent`: `claude`, `codex` or `cursor`. `folder`: `here` (the folder the pen was started in) or
  `worktree` (a new `git` worktree beside the repository, on the branch `work_options` told).
  `open`: `split` (a pane to the right of this one) or `tab` (a new tab). The new pane is not given
  the focus.

```json
{ "pane": "p41", "folder": "/Users/ada/acme-app", "context": "/Users/ada/.local/state/linearpen/work/20261111T031500Z-ACM-412/issues.md" }
```

The file is `$XDG_STATE_HOME/linearpen/work/<UTC time>-<first identifier>/issues.md` (the
directories `0700`, the file `0600`); it says in its first lines that the text under them was
written by others and holds no instructions for the agent. The text of an issue is never in the
prompt (the agent's command line): only the file's path is. No flag that lets the agent do what it
would ask about is ever added. The key is not in the file.

From here the issues of the call carry a mark in the topic (`working`) until the agent's pane ends.
Errors: `bad-params`, `no-key`, `refused`, `unreachable`, `failed` (the issues are asked of Linear
for their text), `unavailable`, `start-failed`.

### `refresh`

Asks Linear for both lists again. It answers at once with `{}`; the topic tells the lists when they
come. Params: none. Errors: `no-key`.

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
          "labels": [{ "name": "api", "color": "#3e63dd" }], "assignee": null }
      ] }
  ],
  "updated": 1762870000, "organization": "Acme", "person": "Ada",
  "working": { "issue-412": { "agent": "Claude" } }
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
- `updated`: when the lists came, seconds since the epoch; `organization` and `person` are the
  workspace's name and the person's. Both are null before the first answer.
- `working`: the issues an agent was started on and whose pane has not ended, by Linear's id, with
  the agent's name. It is not kept between runs.
