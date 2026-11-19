# The command line

`linearpen COMMAND` reads and writes Linear without the interface. `linearpen help COMMAND` says the
options of each command; this file is what `--json` prints. Text that Linear sent (titles, names,
descriptions, comments) is in the JSON as it came: JSON escapes every control character.

A command ends with 0 when it is done, 1 where Linear could not do it, said no, or has nothing by a
name, 2 where the command line is not understood, a file given with `--attach` cannot be sent, or there
is no usable key, and 3 where a write was sent and it is not known whether Linear made it. On an error
the words are on stderr and stdout is empty.

An issue is given as its identifier (`ACM-412`) or as Linear's id of it, a UUID.

An agent that Work on started uses the same `linearpen` as anyone: it has no variable of its own and
no command it may not run: it makes any write the key allows (a comment, a change, a new issue, files
added to a description), and what it asks the person before running is for its own permission prompts.
The context file Work on writes for it tells whether its issue has files and how to have them
(`show --files`).

## Files

A file in a description or a comment is Markdown. A picture is `![name](address)`, and a file of any
other kind is a link, `[name](address)`, to an address of `https://uploads.linear.app/` (what Linear
makes of a file that was attached to an issue; a link to another host is words, not a file).

`show KEY-N` prints each where it stands as `[image N: name]` (a picture) or `[file N: name]` (a link).
N counts from 1 in the order of the issue, the description first and then the comments, pictures and
files in one count; with no name the name is the file name of the address, or `image` or `file`. A list
ends the text:

```
Files
  1  description  https://uploads.linear.app/ws/9a1f/checkout-error.png
  2  comment by Deniz  https://uploads.linear.app/ws/9a1f/build.log
```

With files in the issue and no `--files`, the last line is the way to have them, once:
`2 files: linearpen show ACM-412 --files DIR saves them`.

`show KEY-N --files DIR` has the files that are on Linear and writes each in DIR, and the list gives the
file of each in place of its address, so that they can be opened. Only addresses of
`https://uploads.linear.app/` are fetched, with the key (it goes to that host and to no other). A redirect
is followed by linearpen and not by `curl`: one step at a time, over https only (any other address ends
it), at most five requests in all, and the key goes in a step only where its address is on
`uploads.linear.app`, so a file Linear sends on to another host is asked for there without it. `curl` is
always run as `curl -q --config -`, so that no `.curlrc` is read. Any other address is told as it is. A file is of any kind and at
most 100 MiB; it is fetched into a file of its own that only the program can read and written to DIR from
there, so that none is held whole in memory. DIR is made, with mode 0700, if it is not there. A file is
named `<N>-<name>.<extension>`: the name is the name in the Markdown or the name in the address, cleaned
to letters, digits, dots, dashes and underscores; the extension is the kind the bytes are (png, jpg, gif,
webp, pdf), else the one the name has, else none. A file of that name that holds the same bytes is the one
used, so that a second run prints the same paths; one that holds other bytes makes the name
`<N>-<name>-2.<extension>`, then `-3`, and nothing is written over. A file that cannot be had is told in
its line, with its reason and its address:

```
  3  description  not on Linear  https://example.com/x.png
```

The reasons are `not on Linear` (not an address of `uploads.linear.app`), `too large` (over 100 MiB),
`could not be fetched`, `too many files` (a run saves at most 100 files) and `too much in all` (and at
most 1 GiB together: a file that would take it over is not kept). The command still ends with 0: the
issue was shown.

`show KEY-N --json` keeps `description` and each comment's `body` as they came and adds `files`, one
for each picture or file, in the same order (`[]` where there is none):

```json
"files": [
  { "n": 1, "kind": "image", "name": "Checkout error", "url": "https://uploads.linear.app/ws/9a1f/checkout-error.png",
    "in": "description", "comment": null, "path": "/tmp/files/1-checkout-error.png", "refused": null }
]
```

`kind` is `image` (written `![]()`) or `file` (written `[]()`); `in` is `description` or `comment`;
`comment` is the place of that comment in `comments` (from 0) and `null` for the description. `path` is
the file where `--files` was given and the file was had, else `null`; `refused` is one of the reasons above
where `--files` was given and it could not be had, else `null`.

### Sending files

`comment KEY-N [--body TEXT] [--attach PATH]` (one of the two is needed), `create` with `--attach PATH`
and `update KEY-N --attach PATH` send files. `--attach` is given once for each file, ten at most. PATH
is a file of any kind, not empty, at most 100 MiB. The files are looked at before Linear is asked
anything (they exist, are files, and are in range); one that cannot be sent ends the command with 2. Each
is copied, as it is sent, into a file of its own that only the program can read, so that what was looked
at is what is sent, and the copy is removed afterwards; the file is told to be what its first bytes say it
is (`image/png`, `image/jpeg`, `image/gif`, `image/webp` or `application/pdf`, else
`application/octet-stream`), never by its name.

Each file is put in Linear's storage first, and then its line is added after the words, with a blank
line before: `![name](address)` for a picture (PNG, JPEG, GIF, WebP) and `[name](address)` for any other
file. It goes at the end of the comment, at the end of the description of the issue made, or, for
`update`, at the end of the description Linear holds now (it is read whole, so that nothing in it is lost,
and only the lines of the files are added). `update --attach` is for one issue and is the only option of
its update. If a file cannot be put in storage, nothing is written and the command ends with 1. Where the
write itself was sent and its answer was lost, the command ends with 3, as for any write; look in Linear
before running it again. The text it prints is what it printed before, with `with 2 files` at the end.

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
  "comments_truncated": false,
  "files": [ <file> ]
}
```

`parent` is `null` where there is none; `<neighbour>` is `{ identifier, title, state }`. `tie` is
`blocks`, `blocked_by`, `related`, `duplicate_of` or `duplicated_by`. The first 50 comments, oldest
first, are given, and `comments_truncated` says there are more in Linear. `<file>` is described under
Files, above.

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
