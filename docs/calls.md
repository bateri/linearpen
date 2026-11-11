# The page and the program

The page (`src/view/`) and the program (`src/program/`) talk only through what is written here:
calls the page makes, and a topic it listens to. A change to one starts in this file. The code that
answers is `src/program/api/`; the page's own side is `src/view/parts/`.

**Units.** Time is seconds since the epoch where it is a moment. Members are `camelCase`.

**Errors.** A call that cannot be done answers with an error: `bad-params` (the params are not what
the call takes: a member it does not know, a value out of range; the words say which), `not-found`
(no such call), `internal` (the program failed), `no-key` (there is no key to ask Linear with),
`refused` (Linear did not accept the key), `unreachable` (Linear was not reached: `curl` is not
installed, or the network, the name or the time failed) and `failed` (Linear answered, but not with
what was asked). The words of an error are for the person and never hold the key.

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
  "updated": 1762870000, "organization": "Acme", "person": "Ada"
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
