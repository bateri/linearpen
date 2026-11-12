-- A new issue: beside the list (or over it, in a narrow pane) a form with what Linear's own asks: a
-- title, what it is about, and the values of the issue (team where there are several, status,
-- priority, assignee, labels, project). Each value is a control that says its value and opens the
-- host's menu of the others. The program does the work (`new_options`, `create`); the page asks it
-- what can be chosen and what the person chose.
--
-- The draft stays when the form is put away (the words, the values) and goes when the issue is made;
-- the team and the project are what the next draft begins with. Making the issue is sent once: the
-- draft has an id that the program gave, and the one that was sent is the one that is sent again
-- where the person presses Create again after a failure.

local kit = require("../lib/kit")
local commands = require("./commands")
local head = require("./head")
local list = require("./list")
local panel = require("./panel")
local values = require("./values")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local new = {}

local TITLE = "new-title"
local TITLE_FIELD = u:name(TITLE, "field")
local VALUE = "new-value:"
local MENU = "new-menu"
local SCROLL = "new-scroll"

-- The values, in the order they stand and Tab goes.
local KINDS = { "team", "state", "priority", "assignee", "labels", "project" }

-- How long a made issue is waited for in the lists that come, seconds.
local WAIT = 30

-- The draft: whether the form is shown; whether there is a draft (the form was opened and its issue
-- is not made); the words of the description while the form is put away; what the program said can be
-- chosen, for the team; the values; whether Create is on its way; why the last did not make it.
local N = {
	open = false,
	live = false,
	serial = 0,
	title = { value = "" },
	about = "",
	restoring = false,
	draft = nil,
	hint = nil,
	team = nil,
	asked = nil,
	options = nil,
	state = nil,
	priority = "none",
	assignee = nil,
	labels = {},
	project = nil,
	creating = false,
	redraw = false,
	error = nil,
	carry = {},
	pending = nil,
}
new.state = N

-- The editor of the description: another for each draft, so that the next begins empty.
local function about_name()
	return "new-about" .. N.serial
end

function new.is_open()
	return N.open
end

-- What the program said can be chosen, once it has.
local function chosen()
	return N.options ~= nil and N.options.status == "ready" and N.options.data or nil
end

local function trimmed(words)
	return string.match(words, "^%s*(.-)%s*$") or ""
end

-- The words of the description: what the editor holds (what was kept, where it has not been made yet
-- or has not been given what was kept).
local function about_words()
	if N.restoring then
		return N.about
	end
	local name = about_name()
	local length = b.layout.editor_length(name)
	if length == nil then
		return N.about
	end
	return b.layout.editor_text(name, 1, length + 1) or ""
end

local find, me_of = values.find, values.me_of

-- --- What can be chosen ------------------------------------------------------------------------------

-- What the draft holds, made to fit what the team offers: what the team has not is let go (a state, a
-- project, a label of another team), and what is not chosen yet begins as it should (the team's own
-- state; the person as the assignee).
local function reconcile(data)
	N.team = data.team
	if N.draft == nil then
		N.draft = data.draft
	end
	if find(data.states, N.state) == nil then
		N.state = data.defaultState
	end
	local kept = {}
	for _, id in N.labels do
		if find(data.labels, id) ~= nil then
			kept[#kept + 1] = id
		end
	end
	N.labels = kept
	if find(data.projects, N.project) == nil then
		N.project = nil
	end
	if N.assignee == "me" or (N.assignee ~= nil and find(data.members, N.assignee) == nil) then
		local me = me_of(data)
		N.assignee = me ~= nil and me.id or nil
	end
end

-- Asks the program what can be chosen for the team (`team`: its id or its key, or nil for the first).
local function ask(team)
	local want = team or ""
	if N.asked == want and N.options ~= nil and N.options.status ~= "failed" then
		return
	end
	N.asked = want
	N.options = { status = "loading" }
	local params = {}
	if team ~= nil then
		params.team = team
	end
	kit.call("new_options", params, function(result, err)
		if N.asked ~= want then
			return true
		end
		if err then
			N.options = { status = "failed", error = err.message or err.code }
			return true
		end
		N.options = { status = "ready", data = result }
		reconcile(result)
		return true
	end)
end

-- --- The draft ---------------------------------------------------------------------------------------

-- The key of the team of the issue the cursor is on (`ACM-412`: `ACM`), or nil.
local function cursor_team()
	local item = list.current()
	if item == nil or item.kind ~= "issue" then
		return nil
	end
	return string.match(item.row.identifier, "^(.-)%-%d+$")
end

-- A draft that begins: empty, as the last one's team and project, or as the cursor's team.
local function begin()
	N.live = true
	N.serial += 1
	N.title.value = ""
	N.about, N.restoring = "", false
	N.draft = nil
	N.hint = N.carry.team or cursor_team()
	N.team, N.asked, N.options = nil, nil, nil
	N.state, N.priority, N.assignee, N.labels = nil, "none", "me", {}
	N.project = N.carry.project
	N.error = nil
end

-- Opens the form, on the draft there is or a new one. The reply that gives the keyboard to the title.
function new.show()
	if N.open then
		return true
	end
	panel.close()
	if not N.live then
		begin()
	end
	N.open = true
	N.restoring = N.about ~= ""
	if N.options == nil or N.options.status ~= "ready" then
		ask(N.team or N.hint)
	end
	N.redraw = true
	-- The title is not drawn yet: the keyboard is asked to go to a field the host will have, which it
	-- does when the field is there (the kit would otherwise look for it a few frames and give up).
	u:field(TITLE_FIELD)
	return u:focus(TITLE_FIELD)
end

-- Puts the form away and keeps the draft.
function new.close()
	if not N.open then
		return
	end
	N.about = about_words()
	N.open = false
	N.restoring = false
end

-- The description is put back in the editor the host has made, with the caret after it.
function new.ready(event)
	if not N.restoring or event.editor ~= about_name() then
		return nil
	end
	N.restoring = false
	if N.about == "" then
		return nil
	end
	return { editor = { [about_name()] = { { load = N.about }, { command = "move", movement = "document_end" } } } }
end

-- --- Making the issue --------------------------------------------------------------------------------

-- Whether Create can be pressed now.
function new.creatable()
	return N.open and not N.creating and chosen() ~= nil and N.draft ~= nil and trimmed(N.title.value) ~= ""
end

-- Why Create cannot be pressed, where it cannot.
local function why_not()
	if N.creating then
		return nil
	elseif chosen() == nil then
		return "Wait for the choices"
	elseif trimmed(N.title.value) == "" then
		return "Give it a title first"
	end
	return nil
end

-- The draft is made: what the next begins with is kept, the rest goes.
local function finish()
	N.carry = { team = N.team, project = N.project }
	N.live = false
	N.draft = nil
	N.title.value = ""
	N.about, N.restoring = "", false
	N.error = nil
end

-- Sends the draft to the program. What comes back is the issue made (shown in the list when the
-- lists have it) or why not (said in the form, with what was written kept).
function new.create()
	if not new.creatable() then
		return true
	end
	local data = chosen()
	local params = { id = N.draft, team = N.team, title = trimmed(N.title.value) }
	local words = about_words()
	if trimmed(words) ~= "" then
		params.description = words
	end
	if N.state ~= nil then
		params.state = N.state
	end
	if N.priority ~= "none" then
		params.priority = N.priority
	end
	if N.assignee ~= nil then
		params.assignee = N.assignee
	end
	if #N.labels > 0 then
		params.labels = table.clone(N.labels)
	end
	if N.project ~= nil then
		params.project = N.project
	end
	-- Where the issue will be: the list of the person's own where it is theirs, and the group of its
	-- state (`kind:name`, as the program names the groups).
	local me = me_of(data)
	local mine = N.assignee ~= nil and me ~= nil and N.assignee == me.id
	local state = find(data.states, N.state)
	local group = state ~= nil and (state.kind .. ":" .. state.name) or nil
	N.creating, N.error = true, nil
	local serial = N.serial
	kit.call("create", params, function(result, err)
		N.creating = false
		if err then
			N.error = err.message or err.code
			-- What was typed while it was on its way is not what was sent: the title field is made to
			-- hold what was.
			N.redraw = true
			return true
		end
		local shown = N.open and N.serial == serial
		finish()
		if not shown then
			-- The form was put away while it was on its way: it is not opened again, and the issue is
			-- in the lists when they come.
			return true
		end
		N.open = false
		-- (Where All issues is shown the issue is in it, whoever it is for.)
		local kind = (S.scope == "mine" and mine) and "mine" or "all"
		N.pending = { id = result.id, kind = kind, expires = S.now + WAIT }
		-- The list is looked at as it is where the issue will be: no filter, the list it is in, its
		-- group open.
		head.field.value = ""
		if group ~= nil then
			S.closed[group] = false
		end
		act.view({ list = { kind = kind }, filter = "", closed = S.closed })
		return u:focus(nil)
	end)
	return true
end

-- The lists came: the issue that was made is shown where it is in them, the cursor on it and the issue
-- open beside the list.
function act.listed()
	local pending = N.pending
	if pending == nil then
		return
	end
	if S.now > pending.expires then
		N.pending = nil
		return
	end
	-- (A list that is not the one the issue is waited for in, or that is on its way, is not looked in.)
	if S.scope ~= pending.kind or S.issues.listing ~= "ready" then
		return
	end
	local item = list.reveal_issue(pending.id)
	if item ~= nil then
		N.pending = nil
		act.open_issue(item)
	end
end

-- --- The values --------------------------------------------------------------------------------------

-- What the menus mark: what the draft holds.
local function current()
	local labels = {}
	for _, id in N.labels do
		labels[id] = true
	end
	return {
		state = N.state,
		priority = N.priority,
		assignee = N.assignee or "none",
		labels = labels,
		project = N.project or "none",
	}
end

-- Opens the host's menu of a value, under its control. The reply that asks for it.
local function open_menu(kind)
	local data = chosen()
	if data == nil or N.creating then
		return true
	end
	local choices = values.choices(kind, data, current(), data.states)
	if #choices == 0 then
		return true
	end
	local entries = values.entries(choices, function(choice)
		return choice.kind .. ":" .. choice.id
	end)
	return u:menu(MENU, entries, { near = VALUE .. kind }) or true
end

-- What a pick of the menu chose.
local function choose(entry)
	local at = string.find(entry, ":", 1, true)
	if at == nil then
		return
	end
	local kind, id = string.sub(entry, 1, at - 1), string.sub(entry, at + 1)
	if kind == "team" then
		if id ~= N.team then
			N.team = id
			-- What is not the other team's is let go when its choices come.
			ask(id)
		end
	elseif kind == "state" then
		N.state = id
	elseif kind == "priority" then
		N.priority = id
	elseif kind == "assignee" then
		N.assignee = id ~= "none" and id or nil
	elseif kind == "label" then
		local kept, found = {}, false
		for _, held in N.labels do
			if held == id then
				found = true
			else
				kept[#kept + 1] = held
			end
		end
		if not found then
			kept[#kept + 1] = id
		end
		N.labels = kept
	elseif kind == "project" then
		N.project = id ~= "none" and id or nil
	end
end

-- --- Drawing -----------------------------------------------------------------------------------------

-- What the chips say: what the draft holds, as the team's choices name it.
local function shown_of(data)
	local labels = {}
	-- In the order they were chosen.
	for _, id in N.labels do
		local label = find(data.labels, id)
		if label ~= nil then
			labels[#labels + 1] = label
		end
	end
	local assignee = find(data.members, N.assignee)
	return {
		team = #data.teams > 1 and (find(data.teams, data.team) or { key = "Team" }) or nil,
		state = find(data.states, N.state),
		priority = N.priority,
		assignee = assignee ~= nil and { id = assignee.id, name = assignee.me and "Me" or assignee.name } or nil,
		labels = labels,
		project = find(data.projects, N.project),
	}
end

local function value_chips(data)
	return values.chips(shown_of(data), {
		prefix = VALUE,
		disabled = N.creating,
		labels = #data.labels > 0,
		projects = #data.projects > 0,
	})
end

-- The title: a line, the host's field in the form's look.
local function title_box()
	if N.redraw then
		-- The field is drawn again after the form was put away, and what the host kept of it then is
		-- not known (it may hold what it did, or nothing): the kit is told it does not know either, so
		-- that the title the draft has is put in it, whatever it was.
		N.redraw = false
		u:field_text(TITLE_FIELD, false)
	end
	return b.div():no_shrink():child(u:input(TITLE, {
		state = N.title,
		label = "Title",
		hint = "Title",
	}))
end

-- The description: an editor the host keeps, that wraps, in a box of the field's look.
local function about_box(compact)
	local t = u.theme
	local name = u:field(about_name())
	local ringed = u:keyboard() == name
	local editor = b.editor(name, {
		wrap = true,
		gutter = false,
		padding = 8,
		line_height = 1.5,
		hint = "Description",
		read_only = N.creating,
		-- Tab is the page's: it goes on to the next field.
		keymap = { tab = false, ["shift+tab"] = false },
	}):grow(1):min_h(0):label("Description")
	return b.div()
		:h(compact and 58 or 104)
		:no_shrink()
		:rounded(t.radius.control)
		:bg(t.field)
		:border(1, ringed and t.ring or t.line)
		:motion({ seconds = t.motion, color = true })
		:child(editor)
end

function new.view(width)
	local compact = S.height < SIZE.short
	local data = chosen()
	local content = b.div():col():grow(1):min_w(0):min_h(0):pt(SIZE.head_top):px(SIZE.pad):gap(compact and 8 or 12)
	content:child(
		b.div():row():items_center():gap(8):no_shrink():child(
			b.text("New issue"):text_size(15):weight(700)
		):child(b.spacer()):child(kit.button("new-close", "Close", { suffix = "Esc" }))
	)
	content:child(title_box())
	content:child(about_box(compact))
	if N.options == nil or N.options.status == "loading" then
		content:child(b.div():row():items_center():gap(8):no_shrink():child(kit.spinner(14, "quiet")):child(
			b.text("Loading the choices"):color("dim"):text_size(SIZE.small)
		))
	elseif data == nil then
		content:child(b.text(N.options.error or "The program did not say what can be chosen"):color("warning"):wrap():text_size(SIZE.small))
		content:child(kit.button("new-retry", "Try again"))
	else
		content:child(value_chips(data))
	end
	if N.error ~= nil then
		content:child(
			b.div():row():items_start():gap(8):no_shrink():child(
				b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)
			):child(b.text(N.error):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
		)
	end
	local reason = why_not()
	content:child(b.div():row():no_shrink():child(kit.button("new-create", "Create issue", {
		primary = true,
		suffix = commands.label("create"),
		busy = N.creating,
		disabled = not new.creatable(),
		tip = reason,
	})))
	local side = b.div():row():w(width):no_shrink():min_h(0)
	if width < S.width then
		side:child(b.div():w(1):bg(b.alpha("foreground", 0.23)):no_shrink())
	end
	return side:child(b.div():id(SCROLL):scroll():col():grow(1):min_w(0):min_h(0):pb(SIZE.pad):child(content))
end

-- --- What the person does ----------------------------------------------------------------------------

-- Whether the keyboard is on one of the values (where Enter chooses).
function new.on_value()
	local target = u:target()
	return N.open and target ~= nil and string.sub(target, 1, #VALUE) == VALUE
end

-- A press on one of the form's own buttons or values. The reply, or nil where it was not the form's.
function new.press(id)
	if id == "new-close" then
		new.close()
		return u:focus(nil)
	elseif id == "new-create" then
		return new.create()
	elseif id == "new-retry" then
		ask(N.team or N.hint)
		return true
	elseif string.sub(id, 1, #VALUE) == VALUE then
		return open_menu(string.sub(id, #VALUE + 1))
	end
	return nil
end

-- What the title or a menu did. The reply, or nil where it was not the form's.
function new.action(action)
	local from, kind = action.from, action.kind
	if from == TITLE then
		if kind == "change" then
			if not N.creating then
				N.title.value = action.text
			end
		elseif kind == "submit" then
			if action.mods ~= nil and action.mods.command then
				return new.create()
			end
			-- Enter in the title goes on to the description; it does not make the issue.
			return u:focus(about_name())
		end
		return true
	elseif from == MENU and kind == "menu" then
		choose(action.entry)
		return true
	end
	return nil
end

-- What Tab goes through, in order, as it stands now: the title, the description, the values that are
-- drawn, Create and Close.
local function order()
	local names = { TITLE_FIELD, u:field(about_name()) }
	for _, kind in KINDS do
		local name = VALUE .. kind
		if b.layout.bounds(name) ~= nil then
			names[#names + 1] = name
		end
	end
	for _, name in { "new-create", "new-close" } do
		if b.layout.bounds(name) ~= nil then
			names[#names + 1] = name
		end
	end
	return names
end

-- Tab (and Shift Tab) goes on to the next field of the form, and round. The reply, or nil where the
-- key is not Tab.
function new.tab(event)
	local mods = event.mods or {}
	if event.key ~= "tab" or mods.command or mods.control or mods.alt then
		return nil
	end
	local names = order()
	local at = 0
	local now = u:target()
	for index, name in names do
		if name == now then
			at = index
		end
	end
	local step = mods.shift and -1 or 1
	return u:focus(names[((at - 1 + step) % #names) + 1])
end

-- Esc, which the form takes whoever holds the keyboard.
function new.key(event)
	local mods = event.mods or {}
	if event.key == "escape" and not (mods.command or mods.control or mods.alt or mods.shift) then
		new.close()
		return u:focus(nil)
	end
	return nil
end

return new
