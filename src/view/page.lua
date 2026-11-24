-- The page of linearpen: the issues of Linear, read, in the pane. The program asks Linear and keeps
-- the model (the groups, the filter); the page draws the lines in view, keeps the cursor and the
-- selection and tells the program what it looks at. It talks to the program only by the calls and the
-- topic of docs/calls.md, and draws with the kit's library (`@pencere/ui`).

local markdown = require("@pencere/markdown")
local kit = require("./lib/kit")
local attach = require("./parts/attach")
local change = require("./parts/change")
local comment = require("./parts/comment")
local commands = require("./parts/commands")
local context = require("./parts/context")
local data = require("./parts/data")
local edit = require("./parts/edit")
local filters = require("./parts/filters")
local head = require("./parts/head")
local help = require("./parts/help")
local list = require("./parts/list")
local lists = require("./parts/lists")
local new = require("./parts/new")
local palette = require("./parts/palette")
local picker = require("./parts/picker")
local panel = require("./parts/panel")
local pictures = require("./parts/pictures")
local screens = require("./parts/screens")
local status = require("./parts/status")
local work = require("./parts/work")

local b, u, S, SIZE, act, lib = kit.b, kit.u, kit.S, kit.SIZE, kit.act, kit.lib

-- --- The commands -----------------------------------------------------------------------------------

-- What a command can be done under: what the page shows, never where the keyboard is, but for the one
-- whose key a field would take (`idle`).
local function modal()
	return palette.is_open() or help.is_open() or lists.is_open() or pictures.is_open() or picker.is_open()
end

-- While a form (the one for an agent, the one for a new issue) is open, or the title of the issue is
-- being written, its field has the keyboard, and a command that is a plain key (a letter, a digit, an
-- arrow) is not done: the key is the field's. ⌘K and ⌘Enter are not letters.
local function form_open()
	return work.is_open() or new.is_open() or panel.editing()
end
-- Where files are put: the key of the place (`attach.lua`). The comment, while its field has the
-- keyboard; the issue being made, while its form is open; else the description of the issue that is open
-- (not while a form for an agent has the pane, nor while the title is written). Nil where there is none.
local function attach_key()
	if S.screen ~= "list" or modal() or work.is_open() then
		return nil
	end
	if new.is_open() then
		return "new"
	end
	local detail = panel.detail()
	if detail == nil or change.editing() then
		return nil
	end
	if comment.typing() then
		return "comment:" .. detail.id
	end
	-- (The edit of a comment takes no file: a file goes in a comment of its own.)
	if edit.typing() and not edit.typing_description() then
		return nil
	end
	return "describe:" .. detail.id
end

-- Where a paste of files goes: the place of `attach_key`. A comment that is edited takes none, and says so.
local function paste_key()
	local key = attach_key()
	if key == nil and edit.typing() and not edit.typing_description() then
		kit.toast("A file goes in a comment of its own")
	end
	return key
end

-- The key of the place a kind of Attach file (`comment`, `new` or `describe`) is of: its own, whatever the
-- keyboard is on.
local function attach_key_of(kind)
	if S.screen ~= "list" or modal() or work.is_open() then
		return nil
	end
	if kind == "new" then
		return new.is_open() and "new" or nil
	end
	local detail = panel.detail()
	if detail == nil or new.is_open() then
		return nil
	end
	return kind .. ":" .. detail.id
end

-- The kind of place the key of Attach file is for now (the button that has the key written in it).
act.attach_kind = function()
	local key = attach_key()
	if key == nil then
		return nil
	end
	return string.match(key, "^(%a+)")
end

-- Whether the point (x, y) is in the element named `name`.
local function inside(name, x, y)
	local at = b.layout.bounds(name)
	return at ~= nil and x >= at.x and x < at.x + at.w and y >= at.y and y < at.y + at.h
end

-- Whether the point is in the room the issue is read in: what is scrolled out of it is not under the
-- pointer, whatever the box of it says.
local function in_the_room(x, y)
	local at = act.panel_view()
	return at ~= nil and x >= at.x and x < at.x + at.w and y >= at.y and y < at.y + at.h
end

-- The place a drop at (x, y) is on: the form of a new issue anywhere on it, or the comment's field and its
-- pictures, or the description of the issue open with what is under it.
local function drop_key(x, y)
	if S.screen ~= "list" or modal() or work.is_open() then
		return nil
	end
	if new.is_open() then
		return inside("new-form", x, y) and "new" or nil
	end
	local detail = panel.detail()
	if detail == nil then
		return nil
	end
	if not in_the_room(x, y) then
		return nil
	end
	if inside("comment-box", x, y) then
		return "comment:" .. detail.id
	elseif inside("describe-zone", x, y) then
		return "describe:" .. detail.id
	end
	return nil
end

commands.when.list = function()
	return S.screen == "list" and not modal() and not form_open()
end
-- The card of keys is not offered where words are written in a form (`?` is a letter of them): the form
-- of a new issue, and the words for the agent in the form of Work on (on its choices it is offered).
commands.when.helpable = function()
	return S.screen == "list" and not modal() and not new.is_open() and not panel.editing() and not work.typing()
end
-- (With a form open too: Refresh asks again for what the form offers, a team made in Linear.)
commands.when.keyed = function()
	return (S.screen == "list" or S.screen == "failed") and not modal()
end
commands.when.chord = function()
	return S.screen == "list" and not modal()
end
commands.when.idle = function()
	return commands.when.list() and not head.typing()
end
-- The list is on the page: what selects its lines, folds its groups, filters it or chooses which list
-- it is. (Where the issue has the whole pane it is not.)
commands.when.shown = function()
	return commands.when.list() and not S.alone
end
commands.when.shown_idle = function()
	return commands.when.idle() and not S.alone
end
-- What the head does (the list that is shown, the filter, the row of the filters): the head is the
-- list's, and where the issue has the whole pane it is not on the page, and its keys do nothing. (Where
-- a form has it, no key of the list is the form's anyway.)
commands.when.headed = function()
	return commands.when.list() and not S.alone
end
commands.when.headed_idle = function()
	return commands.when.idle() and not S.alone
end
-- The one issue the copies, Open in Linear, the title and the comment are of: the issue read where one is
-- open, else the one selected or the one under the cursor, where none of several is. (The copies are
-- chords and a field does not take them.)
commands.when.single = function()
	return commands.when.list() and context.single() ~= nil
end
commands.when.single_idle = function()
	return commands.when.idle() and context.single() ~= nil
end
-- Space scrolls the open issue, but where a button has the keyboard it presses the button.
commands.when.reading = function()
	local target = u:target()
	return commands.when.idle() and panel.is_open() and not (target ~= nil and kit.buttons[target] ~= nil)
end
-- The keys that change the issues the cursor is on, or those selected: there is one to change, and no
-- change of it is on its way.
commands.when.changeable = function()
	return commands.when.idle() and change.can()
end
-- The priority and the due date need no choices of the team.
commands.when.plain_change = function()
	return commands.when.idle() and change.can(true)
end
-- The cycle and the estimate need the team to have them: not for the issues of one that has none.
commands.when.cyclable = function()
	return commands.when.changeable() and change.offers("cycle", change.targets())
end
commands.when.estimable = function()
	return commands.when.changeable() and change.offers("estimate", change.targets())
end
-- The title is of one issue (the one read, where one is), and needs none either.
commands.when.titleable = function()
	return commands.when.idle() and change.single_can() and not panel.pull_open()
end
-- The parent and the other neighbours are the open issue's, as it tells them (they are not offered
-- where it has none).
-- (The issue's keys are not for the pull request shown in its place: what they act on is not drawn.)
commands.when.parented = function()
	return commands.when.idle() and panel.has_parent() and not panel.pull_open()
end
commands.when.readable = function()
	return commands.when.idle() and act.unread(context.single())
end
commands.when.pulled = function()
	return commands.when.idle() and panel.pulls() > 0 and not panel.pull_open()
end
commands.when.pull_shown = function()
	return commands.when.idle() and panel.pull_open()
end
commands.when.neighboured = function()
	return commands.when.idle() and panel.neighbours() > 0 and not panel.pull_open()
end
-- The comment is the open issue's: its field is at the end of its comments, and the words in it are
-- sent by ⌘Enter while the field has the keyboard, and by the press of Comment (a form has ⌘Enter for its
-- own, and a key pressed elsewhere sends nothing the person wrote).
commands.when.commentable = function()
	return commands.when.idle() and panel.detail() ~= nil and not panel.pull_open()
end
-- Attach file: where there is a place for files (the key does not need the list's keys, since the field
-- of the comment has the keyboard where it is the comment's).
commands.when.attachable = function()
	return attach_key() ~= nil and not panel.pull_open()
end
-- Add to description: files are chosen for it, and the keyboard is not in a field (⌘Enter there is the
-- field's: the comment is posted, the issue made, the words searched for). The comment's field and the
-- head's (the filter, the search) have the keyboard even when they are empty, and ⌘Enter does not go to
-- the description then.
commands.when.describable = function()
	return S.screen == "list"
		and not modal()
		and not form_open()
		and not comment.typing()
		and not head.typing()
		and attach.describable()
end
-- The words of the issue open can be edited: Linear has told it, and no edit of it is open. The last
-- comment of the person's is edited where there is one.
commands.when.describable_words = function()
	local detail = panel.detail()
	return commands.when.idle() and detail ~= nil and edit.of(detail.id) == nil and not panel.pull_open()
end
commands.when.comment_editable = function()
	local detail = panel.detail()
	return commands.when.list()
		and detail ~= nil
		and edit.of(detail.id) == nil
		and edit.last_mine(detail) ~= nil
		and not panel.pull_open()
end
-- Save the edit: one is open on the issue open and can be saved, and the keyboard is not in another field
-- (⌘Enter there is that field's: the comment is posted, the words searched for).
commands.when.edit_savable = function()
	local detail = panel.detail()
	return S.screen == "list"
		and not modal()
		and not work.is_open()
		and not new.is_open()
		and not change.editing()
		and not comment.typing()
		and not head.typing()
		and detail ~= nil
		and edit.saveable(detail.id)
end
-- Save the file, Open the PDF: a card of the issue open has the keyboard, or is the only one there is.
commands.when.file_openable = function()
	return S.screen == "list" and not modal() and not form_open() and pictures.openable()
end
commands.when.file_linkable = function()
	return S.screen == "list" and not modal() and not form_open() and pictures.linkable()
end
-- The arrows turn the page of the PDF that is large.
commands.when.paging = function()
	return S.screen == "list" and pictures.paging()
end
-- The words of a PDF that is large: to find (it has some), to go from match to match (there are some),
-- to copy (some are selected) and to select all.
local function reading_pdf()
	return S.screen == "list"
		and pictures.is_open()
		and not palette.is_open()
		and not help.is_open()
		and not lists.is_open()
end
-- Save: a card of the issue (the one that has the keyboard, or the only one there is), or the PDF that is
-- large.
commands.when.file_saveable = function()
	if pictures.is_open() then
		return reading_pdf() and pictures.saveable()
	end
	return S.screen == "list" and not modal() and not form_open() and pictures.saveable()
end
commands.when.pdf_readable = function()
	return reading_pdf() and pictures.readable()
end
commands.when.pdf_found = function()
	return reading_pdf() and pictures.found()
end
commands.when.pdf_selected = function()
	return reading_pdf() and pictures.selected()
end
commands.when.postable = function()
	local detail = panel.detail()
	return S.screen == "list"
		and not modal()
		and not work.is_open()
		and not new.is_open()
		and not change.editing()
		and detail ~= nil
		and comment.typing()
		and comment.sendable(detail.id)
end
-- The filters can be let go of where some are set and their row is on the page. (Shift and F is a
-- letter of the words of a field.)
-- The list takes the filters of Linear: every list but the inbox.
commands.when.filterable = function()
	return commands.when.headed_idle() and S.scope ~= "inbox"
end
commands.when.narrowed = function()
	return commands.when.filterable() and filters.any()
end
-- The words of the field can be searched for in all of Linear where the person is typing them, or
-- where nothing held matches them (the card says so, with the key): not in a form or a comment, and
-- not where the words are a filter of a list with issues in it (the key is a comment's, then).
commands.when.searchable = function()
	return commands.when.headed()
		and not comment.typing()
		and head.words() ~= ""
		and (head.typing() or head.unmatched())
end
-- The key 3 to 9 goes to the first to the seventh favorite, where the person has that many.
for number = 3, 9 do
	commands.when["favorite_" .. number] = function()
		return commands.when.headed() and lists.favorite(number - 2) ~= nil
	end
end
-- The agent of the one issue the keys are for, where one works on it.
-- What stands beside the list can be sized: it is beside it (not over it), and no words are being written.
commands.when.sizable = function()
	return commands.when.idle() and S.panel_open and not S.covered and not S.writing and not panel.editing()
end
commands.when.reachable = function()
	return commands.when.idle() and work.agent_of(context.single()) ~= nil
end
-- Work on is the open issue's: where an issue is open (its detail has the button).
commands.when.workable = function()
	return commands.when.idle() and work.workable() and not panel.pull_open()
end
commands.when.startable = function()
	return S.screen == "list" and not modal() and work.startable()
end
commands.when.creatable = function()
	return S.screen == "list" and not modal() and new.creatable()
end

local cmds = lib.commands(commands.for_library())

-- An issue opens beside the list, unless a form is open: that one is not put away by a press on a
-- line.
act.open_issue = function(item)
	if form_open() then
		return true
	end
	return panel.show(item)
end

-- --- Which screen -----------------------------------------------------------------------------------

-- What the page shows: "boot" until the program answers, "connect" where there is no key (or Linear
-- refused the one there is, before any list came), "loading" until the first list, "failed" where it
-- could not come, "list" otherwise.
local function screen()
	if S.boot == nil then
		return "boot"
	end
	local snapshot = S.issues
	local error = snapshot ~= nil and snapshot.error or nil
	if not S.has_key or (error ~= nil and error.kind == "refused" and not snapshot.loaded) then
		return "connect"
	end
	if snapshot == nil or (not snapshot.loaded and error == nil) then
		return "loading"
	end
	if not snapshot.loaded then
		return "failed"
	end
	return "list"
end

local function booting()
	local box = b.div():grow(1):col():items_center():justify_center():gap(8)
	if S.boot_error then
		box:child(b.text("The program did not answer"):weight(650))
		box:child(b.text(S.boot_error):color("dim"):wrap():max_w(520):align("center"))
	else
		box:child(kit.spinner(20, "quiet"))
	end
	return box
end

-- The list's room: the lines, or what stands where they would.
local function body()
	local snapshot = S.issues
	-- A list the person went to: on its way, or one that Linear could not tell.
	if snapshot.listing == "loading" then
		return screens.coming()
	elseif snapshot.listing == "failed" then
		return screens.failed(snapshot.error or { message = "The list could not be asked for" })
	end
	if list.count() == 0 then
		if S.scope == "mine" and head.field.value == "" and not filters.any() and snapshot.counts.mine == 0 then
			return screens.empty()
		end
		return screens.nothing({
			words = head.field.value,
			search = S.scope == "search",
			inbox = S.scope == "inbox",
			-- (The inbox takes no filter: what is set is the other lists'.)
			narrowed = S.scope ~= "inbox" and filters.any(),
		})
	end
	return list.view(S.room, S.panel_open)
end

-- The line that says the last asking failed while the lists of before are still shown.
local function problem()
	local error = S.issues.error
	-- (A list that could not be asked for says why in its own place.)
	if error == nil or S.issues.listing ~= "ready" then
		return nil
	end
	return u:banner("problem", {
		tone = "warn",
		icon = "assets/icons/alert-triangle.svg",
		words = error.message,
		actions = { { key = "retry", title = "Try again" } },
	})
end

-- How long the light of the line that says Linear is asked takes to cross it, seconds; and how much of
-- the line it is.
local SWEEP = 1.2
local LIGHT = 0.3

-- The line along the top of the page (over the list and its head and the issue beside it, or the issue
-- or the form that has the pane) while Linear is asked for the lists again (Refresh, the five minutes,
-- an issue made), and then for the issue open: the same in every width, where the button of Refresh is
-- or is not. It is out of the flow, over the top of them, so nothing moves when it comes or goes. A list
-- that is coming says so in its own place and has none. Its light crosses it, moved by the terminal (a
-- loop: the page is not drawn again for it), or by the page frame by frame where the terminal's kit has
-- no loop. Under Reduce Motion it is lit and still.
local function refreshing()
	if not (S.refreshing or panel.again()) or S.issues.listing ~= "ready" then
		return nil
	end
	local line = b.div():id("refreshing"):absolute():inset(0, 0, nil, 0):h(2):clip():bg(b.alpha("accent", 0.2))
	if u.reduce_motion then
		return line:bg(b.alpha("accent", 0.6))
	end
	local light = S.width * LIGHT
	local lit = b.div():id("refreshing-light"):absolute():size(light, 2):bg("accent")
	if kit.has("motion.loop") then
		return line:child(lit:inset(0, nil, nil, -light):motion({
			loop = { seconds = SWEEP, dx = S.width + light, ease = "linear", alternate = false },
		}))
	end
	u:animate()
	local at = (S.now % SWEEP) / SWEEP * (S.width + light) - light
	return line:child(lit:inset(0, nil, nil, at))
end

-- `row` (what is under the head) with the line over its top while Linear is asked.
local function lined(row)
	local line = refreshing()
	if line ~= nil then
		row:child(line)
	end
	return row
end

-- What stands beside the list: the form for a new issue, the form that hands issues to an agent, or
-- the issue.
local function side(width)
	if new.is_open() then
		return new.view(width)
	end
	if work.is_open() then
		return work.view(width)
	end
	return panel.view(width)
end

-- What stands beside the list (or has the pane), `width` points wide, in a box that fades in as it
-- opens. The box is the same for the issue and the forms, so going from one issue to the next, or from
-- the issue to a form, does not bring it in again. (It does not slide: a menu opened from a value while
-- it slid would stand where the value was on its way, and stay there.)
local function beside(width)
	return b.div()
		:id("side")
		:row()
		:w(width)
		:h_full()
		:min_h(0)
		:no_shrink()
		:motion({ enter = { opacity = 0, seconds = 0.24 } })
		:child(side(width))
end

-- The edge between the list and what stands beside it, the kit's split: dragged (or moved with the arrow
-- keys while it has the keyboard, or with the page's own keys, `commands.lua`) it sizes what stands
-- beside the list, between `SIZE.side_least` and what leaves the list its whole head (`panel.most`). Its
-- state is the width drawn: what the person left it at, held by the page's width (`panel.width`).
local EDGE = "side-edge"
local edge = { size = SIZE.side_least, drawer = false }

-- The list and its head, and beside them the issue or a form from the top of the page to the line of
-- keys, the edge between them. In a pane too narrow for both, the issue or the form has the pane to
-- itself: the head is the list's, and it goes with the list (`commands.when.headed`).
local function listing()
	local page = b.div():col():grow(1):min_h(0)
	if pictures.is_open() then
		-- A picture is large: it has the pane, the head and the list are not on the page.
		return page:child(pictures.view())
	end
	if S.covered then
		return page:child(lined(b.div():row():grow(1):min_h(0):child(beside(S.width))))
	end
	local left = b.div():col():grow(1):min_w(0):min_h(0)
	left:child(head.view(S.room))
	local banner = problem()
	if banner ~= nil then
		left:child(banner)
	end
	local strip = change.strip()
	if strip ~= nil then
		left:child(strip)
	end
	left:child(body())
	local row = b.div():row():grow(1):min_h(0)
	if not S.panel_open then
		return page:child(lined(row:child(left)))
	end
	local width = panel.width(S.width)
	edge.size = width
	return page:child(lined(row:child(u:split(EDGE, {
		state = edge,
		side = "end",
		min = SIZE.side_least,
		max = panel.most(S.width),
		label = "Panel width",
		first = left,
		second = beside(width),
	}))))
end

-- Whether the keyboard is on the edge and `event` is one of its arrows: the plain left and right keys,
-- which the terminal gives as the list's commands of folding a group and opening it.
local function on_the_edge(event)
	if u:target() ~= u:name(EDGE, "handle") then
		return false
	end
	if event.type == "command" then
		return event.id == "fold" or event.id == "unfold"
	end
	local mods = event.mods or {}
	local plain = not (mods.command or mods.control or mods.alt or mods.shift)
	return event.type == "key" and plain and (event.key == "left" or event.key == "right")
end

-- Sizes what stands beside the list by `by` points, as far as it goes, and keeps it. The reply.
local function size_side(by)
	local now = panel.width(S.width)
	local to = math.min(math.max(now + by, SIZE.side_least), panel.most(S.width))
	if to ~= now then
		panel.keep(to)
		act.panel_width(to)
	end
	return true
end

local function render(size, now, reduce_motion)
	u:begin(size, now, reduce_motion)
	pictures.begin()
	attach.begin()
	S.now, S.width, S.height = now, size.w, size.h
	S.screen = screen()
	S.work_open = S.screen == "list" and work.is_open()
	S.new_open = S.screen == "list" and new.is_open()
	S.form_open = S.work_open or S.new_open
	S.editing = S.screen == "list" and change.editing()
	S.writing = S.screen == "list" and (comment.typing() or edit.typing())
	S.panel_open = S.screen == "list" and (panel.is_open() or S.form_open)
	-- The issue or a form has the whole pane (it is too narrow for the list beside it): the list and its
	-- head are not on the page, and no key is theirs.
	S.covered = S.panel_open and S.width < SIZE.beside
	-- The list's room, and its head's: the page's, less what stands beside it and the edge between them.
	S.room = (S.panel_open and not S.covered) and S.width - panel.width(S.width) - SIZE.edge or S.width
	S.alone = S.covered and panel.is_open() and not S.form_open
	local root = b.div():size_full():col():bg("background"):color("foreground"):family("terminal")
	if S.screen == "boot" then
		root:child(booting())
	elseif S.screen == "connect" then
		root:child(screens.connect())
		root:child(status.connect())
	elseif S.screen == "loading" then
		root:child(screens.loading())
		root:child(status.empty())
	elseif S.screen == "failed" then
		root:child(screens.failed(S.issues.error))
		root:child(status.empty())
	else
		root:child(listing())
		-- (The line of keys is as high as it is whatever the list is long: it does not give way.)
		root:child(status.view():no_shrink())
		palette.view(root)
		lists.view(root)
		help.view(root)
		-- (The card of the values is the layer in front: Esc and the keys are its while it is open.)
		picker.view(root)
	end
	if S.screen ~= "list" then
		-- (The list's screen went: the control the card stood by with it.)
		picker.close()
	end
	kit.start_toasts(u.now)
	kit.lib.toasts_prune(S.toasts, u.now)
	if #S.toasts > 0 then
		root:child(u:toasts("toasts", { items = S.toasts }))
	end
	commands.settle()
	cmds:write(u, root)
	return u:finish(root)
end

-- --- What the person does ------------------------------------------------------------------------------

local function command(id)
	if (id == "fold" or id == "unfold") and u:target() == u:name(EDGE, "handle") then
		-- The keyboard is on the edge: its arrows move it, left to widen the issue, a step at a time.
		return size_side(id == "fold" and SIZE.edge_step or -SIZE.edge_step)
	elseif id == "fold" then
		list.fold(true)
	elseif id == "unfold" then
		list.fold(false)
	elseif id == "select" then
		list.toggle_mark(list.state.cursor)
	elseif id == "select_all" then
		list.select_all()
	elseif id == "status" then
		return change.run("state")
	elseif id == "priority" then
		return change.run("priority")
	elseif id == "assignee" then
		return change.run("assignee")
	elseif id == "labels" then
		return change.run("labels")
	elseif id == "project" then
		return change.run("project")
	elseif id == "cycle" then
		return change.run("cycle")
	elseif id == "estimate" then
		return change.run("estimate")
	elseif id == "due" then
		return change.run("due")
	elseif id == "assign_me" then
		local ids = change.targets()
		return change.assign_me(ids)
	elseif id == "title" then
		return panel.rename_target()
	elseif id == "copy_id" then
		return context.copy("id", context.single())
	elseif id == "copy_branch" then
		return context.copy("branch", context.single())
	elseif id == "copy_link" then
		return context.copy("link", context.single())
	elseif id == "share" then
		return context.copy("share", context.single())
	elseif id == "widen" then
		return size_side(SIZE.side_step)
	elseif id == "narrow" then
		return size_side(-SIZE.side_step)
	elseif id == "link" then
		return context.link(context.single())
	elseif id == "comment" then
		return panel.write_comment()
	elseif id == "comment_send" then
		local detail = panel.detail()
		return detail ~= nil and comment.send(detail.id) or true
	elseif id == "edit_description" then
		return panel.edit_words("description")
	elseif id == "edit_comment" then
		return panel.edit_words("comment")
	elseif id == "edit_save" then
		return edit.save(panel.id())
	elseif id == "attach_file" then
		local key = attach_key()
		return key ~= nil and attach.choose(key) or true
	elseif id == "pdf_find" then
		return pictures.find()
	elseif id == "pdf_find_next" then
		return pictures.step(1)
	elseif id == "pdf_find_previous" then
		return pictures.step(-1)
	elseif id == "pdf_copy" then
		return pictures.copy()
	elseif id == "pdf_select_all" then
		return pictures.select_all()
	elseif id == "file_save" then
		return pictures.save_target()
	elseif id == "file_open" then
		return pictures.open_target()
	elseif id == "file_link" then
		return pictures.link_target()
	elseif id == "file_copy_link" then
		return pictures.copy_link_target()
	elseif id == "describe" then
		return attach.describe()
	elseif id == "parent" then
		return panel.open_parent()
	elseif id == "near" then
		return panel.open_neighbours()
	elseif id == "pull_request" then
		return panel.open_pulls()
	elseif id == "pull_github" then
		return panel.pull_on_github()
	elseif id == "mark_read" then
		return act.mark_read(context.single())
	elseif id == "lists" then
		lists.show()
	elseif id == "read_down" then
		panel.scroll(1)
	elseif id == "read_up" then
		panel.scroll(-1)
	elseif id == "scope_mine" then
		act.view({ list = { kind = "mine" } })
	elseif id == "scope_all" then
		act.view({ list = { kind = "all" } })
	elseif string.sub(id, 1, 9) == "favorite_" then
		local go = lists.favorite(tonumber(string.sub(id, 10)) - 2)
		if go ~= nil then
			act.view({ list = go })
		end
	elseif id == "filter" then
		return head.focus()
	elseif id == "filters" then
		return filters.show()
	elseif id == "clear_filters" then
		return filters.clear()
	elseif id == "search" then
		return head.search()
	elseif id == "refresh" then
		act.refresh()
	elseif id == "work" then
		return work.show()
	elseif id == "go_to_agent" then
		return work.go_to(context.single())
	elseif id == "start" then
		return work.start()
	elseif id == "new" then
		return new.show()
	elseif id == "create" then
		return new.create()
	elseif id == "palette" then
		palette.show()
	elseif id == "help" then
		help.show()
	end
	return true
end

act.run = command

-- A button of the page's own pressed (kit.button).
local function press(id)
	if id == "refresh" or id == "retry" then
		act.refresh()
	elseif id == "show-all" then
		act.view({ list = { kind = "all" } })
	elseif id == "connect" then
		act.connect(screens.field.value)
	elseif id == "create-key" then
		return { link = screens.KEYS_URL }
	elseif id == "lists" then
		lists.show()
	elseif id == "search-all" then
		return head.search()
	elseif id == "work" then
		return work.show()
	elseif id == "new" then
		return new.show()
	elseif id == "comment-send" then
		-- (The button can be pressed where there is something to send: the words are posted as they are.)
		local detail = panel.detail()
		return detail ~= nil and comment.send(detail.id) or true
	elseif id == "open-linear" then
		return panel.link()
	elseif id == "panel-share" then
		return context.copy("share", context.single())
	elseif id == "panel-close" then
		panel.close()
	elseif id == "panel-title-edit" then
		return panel.rename()
	elseif id == "picture-close" then
		-- (Pressed with the pointer: the keyboard stays the list's, as after any press, `keyboard_back`.)
		return pictures.close(true)
	elseif string.sub(id, 1, 4) == "pic:" or string.sub(id, 1, 10) == "file-open:" then
		return pictures.open(id)
	elseif string.sub(id, 1, 10) == "file-save:" or id == "pdf-save" then
		return pictures.save(id)
	elseif id == "pdf-find-open" then
		return pictures.find()
	elseif id == "pdf-find-next" then
		return pictures.step(1)
	elseif id == "pdf-find-previous" then
		return pictures.step(-1)
	elseif id == "pdf-find-close" then
		return pictures.close_find()
	elseif id == "pdf-previous" then
		return pictures.turn(-1)
	elseif id == "pdf-next" then
		return pictures.turn(1)
	elseif string.sub(id, 1, 9) == "pic-link:" or string.sub(id, 1, 8) == "pic-ext:" then
		return pictures.link(id)
	elseif string.sub(id, 1, 10) == "file-card:" then
		return pictures.card_press(id)
	elseif string.sub(id, 1, 5) == "near:" then
		return panel.open_near(string.sub(id, 6))
	elseif string.sub(id, 1, 5) == "pull:" then
		return panel.open_pull(tonumber(string.sub(id, 6)))
	elseif id == "pull-back" then
		return panel.close_pull()
	elseif id == "pull-github" then
		return panel.pull_on_github()
	elseif id == "pull-retry" then
		return panel.retry_pull()
	elseif id == "issue-retry" then
		panel.retry()
	elseif id == "clear-selection" then
		list.clear_marks()
	elseif id == "help-close" then
		help.hide()
	else
		local reply = filters.press(id)
		if reply == nil then
			reply = status.press(id)
		end
		if reply == nil then
			reply = work.press(id)
		end
		if reply == nil then
			reply = new.press(id)
		end
		if reply == nil then
			reply = change.press(id)
		end
		if reply == nil then
			reply = attach.press(id, attach_key_of)
		end
		if reply == nil then
			reply = edit.press(id)
		end
		if reply ~= nil then
			return reply
		end
	end
	return true
end

local function on_action(action)
	local from, kind = action.from, action.kind
	local reply = picker.action(action)
	if reply ~= nil then
		return reply
	end
	reply = head.action(action)
	if reply == nil then
		reply = palette.action(action)
	end
	if reply == nil then
		reply = lists.action(action)
	end
	if reply == nil then
		reply = filters.action(action)
	end
	if reply == nil then
		reply = help.action(action)
	end
	if reply == nil then
		reply = context.action(action)
	end
	if reply == nil then
		reply = panel.action(action)
	end
	if reply == nil then
		reply = work.action(action)
	end
	if reply == nil then
		reply = new.action(action)
	end
	if reply == nil then
		reply = change.action(action)
	end
	if reply == nil then
		reply = edit.action(action)
	end
	if reply ~= nil then
		return reply
	end
	if reply == nil then
		reply = pictures.action(action)
	end
	if reply ~= nil then
		return reply
	end
	if from == "toasts" and kind == "close" then
		for index, item in S.toasts do
			if item.key == action.key then
				table.remove(S.toasts, index)
				break
			end
		end
		return true
	elseif from == "status" then
		status.action(action)
		return true
	elseif from == "key" and kind == "submit" then
		act.connect(screens.field.value)
		return true
	elseif from == "problem" and kind == "press" then
		act.refresh()
		return true
	elseif from == "commands" and kind == "command" then
		return command(action.id)
	elseif from == EDGE and kind == "resize" then
		-- The edge goes with the pointer; where it is let go (or a key moved it) the width is kept.
		local width = math.floor(action.size + 0.5)
		panel.keep(width)
		if action.final then
			act.panel_width(width)
		end
		return true
	end
	return true
end

-- Esc closes what is on top: a form (its draft is kept), the row of the filters or the issue, then the
-- selection, then a search (back to the list that was before it), then the words of the filter.
local function back()
	if new.is_open() then
		new.close()
	elseif work.is_open() then
		work.close()
	elseif filters.is_open() then
		filters.close()
	elseif panel.pull_open() then
		-- (The pull request shown goes, and the issue is there again.)
		panel.close_pull()
	elseif edit.cancelable() then
		-- (The edit of the issue open is let go before the issue is put away; where its words were
		-- changed, it asks first.)
		return edit.ask_cancel(panel.id())
	elseif attach.cancel() then
		-- (The files chosen for the description are let go before the issue is put away.)
		return true
	elseif panel.is_open() then
		panel.close()
	elseif list.marked() > 0 then
		list.clear_marks()
	elseif S.scope == "search" then
		return head.leave_search()
	elseif head.field.value ~= "" then
		head.set("")
	else
		return nil
	end
	return true
end

-- A key where the issue has the whole pane: the arrows scroll what is read (the list they would
-- move in is not on the page), Page Up and Page Down by what the room shows, Home and End to its
-- ends; `j` and `k` go to the issue after and the one before. Shift and an arrow, which selects
-- lines, and Enter, which opens the one that is open, do nothing.
local LINES = 63
local function reading_key(event)
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt then
		return nil
	end
	local name = event.key
	if name == "up" or name == "down" then
		if not mods.shift then
			panel.scroll_by(name == "down" and LINES or -LINES)
		end
	elseif name == "page_down" or name == "page_up" then
		panel.scroll(name == "page_down" and 1 or -1)
	elseif name == "home" or name == "end" then
		panel.scroll_by(nil, name == "home" and "top" or "end")
	elseif (name == "j" or name == "k") and not mods.shift then
		list.step_issue(name == "j" and 1 or -1)
	elseif name ~= "enter" then
		return nil
	end
	return true
end

-- A key no component took.
local function key(event)
	local mods = event.mods or {}
	local plain = not (mods.command or mods.control or mods.alt or mods.shift)
	local target = u:target()
	if
		plain
		and (event.key == "enter" or event.key == " ")
		and target ~= nil
		and kit.buttons[target]
		-- (A button that is not drawn any more, such as the one the person pressed and that went with its
		-- screen, is not pressed by a key.)
		and b.layout.bounds(target) ~= nil
	then
		return press(target)
	end
	if S.screen == "list" and help.is_open() then
		return help.key(event)
	end
	if S.screen == "list" and pictures.is_open() and not help.is_open() then
		-- The picture is large: Esc puts it away, and no other key is the list's.
		return pictures.key(event)
	end
	if S.screen ~= "list" or modal() then
		return nil
	end
	if new.is_open() then
		return new.key(event)
	end
	if work.is_open() then
		-- The form takes Tab and Esc whoever holds the keyboard, and the list's keys are not its own.
		return work.key(event)
	end
	local typed = comment.key(event)
	if typed == nil then
		typed = edit.key(event)
	end
	if typed ~= nil then
		return typed
	end
	if panel.editing() then
		-- The title is being written: its keys are the field's (Enter and Esc come as its actions), and
		-- the arrows do not take the cursor from the issue.
		return nil
	end
	if u.holder ~= "page" then
		-- In the field the arrows are the list's and the words stay in it.
		return list.key(event, true) or nil
	end
	if event.key == "escape" then
		return back()
	end
	if S.alone then
		return reading_key(event)
	end
	return list.key(event, false) or nil
end

-- The pointer is over something the page answers itself (a line, a button): the hand.
local function shape(event)
	local over = b.layout.hit({ x = event.x, y = event.y })
	local wanted = nil
	if pictures.over_text(event) then
		-- The words of a PDF that is large are selected.
		wanted = "text"
	elseif
		over ~= nil
		and (
			kit.buttons[over]
			or string.sub(over, 1, 4) == "row:"
			or string.sub(over, 1, 6) == "check:"
			or string.sub(over, 1, 6) == "agent:"
		)
	then
		wanted = "pointer"
	end
	if wanted == S.cursor then
		return nil
	end
	S.cursor = wanted
	return { cursor = wanted or "arrow" }
end

-- The issue open follows the cursor where the cursor is on an issue.
local function follow()
	work.sync()
	change.sync()
	-- The pictures held are the open issue's: they go with it, and come with the one that opens.
	pictures.track(panel.id(), panel.detail())
	if not panel.is_open() then
		return
	end
	local item = list.current()
	-- (The issue the person changed stays open, though the change took it out of the list and the
	-- cursor with it, until the cursor is moved.)
	if item ~= nil and item.kind == "issue" and item.row.id ~= panel.id() and not panel.held() then
		panel.show(item)
	end
end

-- A press of the pointer does not leave the keyboard on the control it pressed: after a click on
-- Refresh, on a part of the scope, on Close or on a value of the issue open, the arrows and Enter are
-- still the list's (Tab is what takes the keyboard to a button). A form, a card and a field (the filter,
-- the title) keep the keyboard they have, and so does a press whose reply sends the keyboard into a
-- field. (The library's own controls put the keyboard on what was pressed, a part of the scope for
-- one, whose arrows would then choose the scope: that is what is taken back.) The reply that gives it
-- back, or nil.
local function keyboard_back(reply)
	if S.screen ~= "list" or modal() or form_open() or u.holder ~= "page" or u:target() == nil then
		return nil
	end
	if type(reply) == "table" and type(reply.keyboard) == "table" then
		return nil
	end
	return u:focus(nil)
end

-- The keyboard on a control of the list's screen is there to press it and for nothing else: Tab
-- brings it, Enter or Space presses. Any other key, and any command, is the list's and takes the
-- keyboard back first, so that the arrows of a part of the scope never choose the scope and Enter
-- after them opens the issue. Esc gives it back and does no more. The edge between the list and the
-- issue is the one control that is moved and not pressed: on it the plain left and right arrows are its
-- own, and size the issue. The reply, and whether the key ends there.
local STAYS = { tab = true, enter = true, [" "] = true, space = true }
local MODS = { shift = true, control = true, alt = true, command = true, meta = true, caps_lock = true }
local function keyboard_from_control(event)
	local kind = event.type
	if kind ~= "key" and kind ~= "command" then
		return nil, false
	end
	if S.screen ~= "list" or modal() or form_open() or u.holder ~= "page" or u:target() == nil then
		return nil, false
	end
	if kind == "key" and (STAYS[event.key] or MODS[event.key]) then
		return nil, false
	end
	local mods = event.mods or {}
	local plain = not (mods.command or mods.control or mods.alt or mods.shift)
	if on_the_edge(event) then
		return nil, false
	end
	-- The keyboard is the list's again: the row of the filters, if it was open for the keyboard, closes.
	filters.close()
	return u:focus(nil), kind == "key" and event.key == "escape" and plain
end

-- Where the issue or a form has the pane, the head is not on the page and the keyboard is not left in
-- it: a pane made narrow while the filter or the row of the filters had it gives it back to the page
-- before the event is heard. The reply, or nil.
local function keyboard_from_head()
	if S.screen ~= "list" or not S.covered then
		return nil
	end
	if filters.is_open() then
		filters.close()
		return u:focus(nil)
	end
	if head.typing() then
		return head.leave()
	end
	return nil
end

local function handle(event)
	local kind = event.type
	-- What is written in the comment and in an edit is kept before the event can take the field from the
	-- page.
	comment.keep()
	edit.keep()
	local left = keyboard_from_head()
	local freed, ends = keyboard_from_control(event)
	freed = b.merge(left, freed)
	if ends then
		return kit.take(freed)
	end
	if kind == "key" and S.screen == "list" and (new.is_open() or work.is_open()) and not modal() then
		local stepped
		if new.is_open() then
			stepped = new.tab(event)
		else
			stepped = work.tab(event)
		end
		if stepped ~= nil then
			return kit.take(stepped)
		end
	end
	local reply, actions, taken = u:handle(event)
	reply = b.merge(freed, reply)
	if kind == "editor_ready" then
		reply = b.merge(reply, new.ready(event))
		reply = b.merge(reply, comment.ready(event))
		reply = b.merge(reply, edit.ready(event))
		reply = b.merge(reply, work.ready(event))
	elseif kind == "edit" then
		reply = b.merge(reply, comment.edited(event))
		reply = b.merge(reply, edit.edited(event))
		reply = b.merge(reply, work.edited(event))
	end
	for _, action in actions do
		reply = b.merge(reply, on_action(action))
	end
	if kind == "move" then
		reply = b.merge(reply, pictures.dragged(event))
		reply = b.merge(reply, shape(event))
	elseif kind == "leave" then
		S.cursor = nil
	elseif kind == "files" then
		-- Files were pasted, dropped or chosen in the Open panel: they are put where they go.
		reply = b.merge(reply, attach.files(event, { paste = paste_key, drop = drop_key }))
	elseif kind == "saving" then
		-- The person chose where a file of Linear is saved: the program writes it there.
		reply = b.merge(reply, pictures.saving(event))
	elseif kind == "carried" then
		-- The system's drag of a line out of the page is over, and the page did not hear the pointer
		-- while it went: what it was told of the pointer (the shape, the button down) is no longer so.
		S.cursor, S.armed, S.away = nil, nil, nil
		reply = b.merge(reply, true)
	elseif kind == "focus" and event.field ~= nil and not picker.holds(event.field) then
		-- The person clicked into a field (the filter's): the row of the filters is put away.
		filters.close()
	elseif kind == "press" then
		if (event.button or "left") == "left" then
			S.armed = b.layout.hit({ x = event.x, y = event.y })
			-- (A press elsewhere puts the row of the filters away, when its button comes up: gone while
			-- the button is down, the row would move what is under it, and the button would come up on
			-- something else.)
			S.away = filters.outside(S.armed)
			pictures.unpoint()
		end
		if not taken and pictures.is_open() then
			-- The page of a PDF that is large: a press on its words begins a selection.
			reply = b.merge(reply, pictures.pressed(event))
		end
		if not taken and S.screen == "list" and not modal() then
			if (event.button or "left") == "right" then
				reply = b.merge(reply, context.press(event) or pictures.menu(event))
			elseif list.press(event) then
				reply = b.merge(reply, true)
			end
		end
		if not taken then
			reply = b.merge(reply, markdown.handle(event))
		end
	elseif kind == "release" then
		reply = b.merge(reply, pictures.released())
		local armed, over = S.armed, b.layout.hit({ x = event.x, y = event.y })
		if S.away then
			filters.close()
		end
		S.armed, S.away = nil, nil
		if
			armed ~= nil
			and armed == over
			and kit.buttons[armed]
			and (event.button or "left") == "left"
		then
			reply = b.merge(reply, press(armed))
		elseif not taken then
			reply = b.merge(reply, markdown.handle(event))
		end
		reply = b.merge(reply, keyboard_back(reply))
	elseif kind == "key" and not taken then
		reply = b.merge(reply, key(event))
	end
	follow()
	return kit.take(reply)
end

-- --- Starting -----------------------------------------------------------------------------------------------

data.start()

return {
	render = render,
	settle = function()
		local moved = list.settle()
		moved = attach.settle() or moved
		moved = edit.settle() or moved
		return u:settle() or moved
	end,
	handle = handle,
}
