# The command line

`linearpen COMMAND` reads and writes Linear without the interface. `linearpen help COMMAND` says the
options of each command; this file is what `--json` prints. Text that Linear sent (titles, names,
descriptions, comments) is in the JSON as it came: JSON escapes every control character.

A command ends with 0 when it is done, 1 where Linear could not do it, said no, or has nothing by a
name, 2 where the command line is not understood or there is no usable key, and 3 where a write was
sent and it is not known whether Linear made it. On an error the words are on stderr and stdout is
empty.

## `list --json`

```json
{
  "list": "My issues",
  "truncated": false,
  "issues": [ <issue>, <issue> ]
}
```

`truncated` is true where Linear has more issues than the 500 that are taken. `update --json` prints
`{ "issues": [ <issue> ] }`, each issue as Linear says it is now; an issue that was changed but that
Linear did not tell again is left out, and stderr names it.

### `<issue>`

```json
{
  "id": "0c9e0f1e-6d1b-4d7a-9a56-3b1c0a5e7f10",
  "identifier": "ACM-412",
  "title": "Add rate limiting to the public API",
  "url": "https://linear.app/acme/issue/ACM-412/add-rate-limiting",
  "state": { "name": "In Progress", "kind": "started" },
  "priority": "high",
  "assignee": { "name": "Ada", "me": true },
  "labels": ["api"],
  "team": "ACM",
  "project": null,
  "cycle": { "number": 12, "name": null },
  "estimate": 3,
  "due_date": "2026-11-20",
  "updated_at": "2026-11-09T14:20:00.000Z",
  "branch_name": "ada/acm-412-add-rate-limiting"
}
```

`state.kind` is `triage`, `backlog`, `unstarted`, `started`, `completed`, `canceled` or `other`;
`priority` is `none`, `urgent`, `high`, `medium` or `low`. `assignee`, `team`, `project`, `cycle`,
`estimate`, `due_date` and `branch_name` are `null` where the issue has none.

## `show --json`

An `<issue>` with these fields besides:

```json
{
  "description": "markdown, or null",
  "parent": { "identifier": "ACM-400", "title": "Billing limits", "state": { "name": "Todo", "kind": "unstarted" } },
  "children": [ <neighbour> ],
  "ties": [ { "tie": "blocked_by", "issue": <neighbour> } ],
  "comments": [ { "author": "Deniz", "created_at": "2026-11-06T09:12:00.000Z", "body": "markdown" } ],
  "comments_truncated": false
}
```

`parent` is `null` where there is none; `<neighbour>` is `{ identifier, title, state }`. `tie` is
`blocks`, `blocked_by`, `related`, `duplicate_of` or `duplicated_by`. The first 50 comments, oldest
first, are given, and `comments_truncated` says there are more in Linear.

## `create --json`

```json
{ "id": "0c9e0f1e-6d1b-4d7a-9a56-3b1c0a5e7f10", "identifier": "ACM-431", "url": "https://linear.app/acme/issue/ACM-431" }
```

## `comment --json`

```json
{ "issue": "ACM-412", "comment": { "author": "Ada", "created_at": "2026-11-10T10:00:00.000Z", "body": "markdown" } }
```

## `teams --json`

Without a key, the teams:

```json
[ { "key": "ACM", "name": "Acme" } ]
```

With one, the names that the options of `create`, `update` and `list` take:

```json
{
  "key": "ACM",
  "name": "Acme",
  "statuses": [ { "name": "Backlog", "kind": "backlog" } ],
  "labels": ["bug", "api"],
  "people": [ { "name": "Ada", "me": true } ],
  "projects": ["Billing"],
  "cycles": [ { "number": 12, "label": "Cycle 12", "running": true } ],
  "estimates": [ { "label": "M", "points": 3 } ]
}
```

`cycles` are the one running and the ones to come; `--cycle` takes a number or `current`. `estimates`
is empty where the team does not estimate.
