-- Changing issues in Linear: one field of one issue or of several (`change`, docs/calls.md). The
-- targets are the issues selected, else the one the cursor is on; the choices are the team's
-- (`new_options`, kept for each team); a choice is sent when it is made, with no question, and
-- the issue is busy until the answer comes. What is written is only what the menus offer: the
-- status, priority, assignee, labels and project, and the title. Nothing is sent again by the page.

local kit = require("../lib/kit")
local list = require("./list")
local values = require("./values")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local change = {}

local MENU = "change-menu"
local TITLE = "panel-title"
local TITLE_FIELD = u:name(TITLE, "field")

-- How many issues one change takes (Linear's limit of a batch).
local MOST = 50

-- The menus' names for what they change.
local TITLES = { state = "Status", priority = "Priority", assignee = "Assignee", labels = "Labels", project = "Project" }

-- The field each kind of choice writes (`change`'s params).
local FIELDS = { state = "status", priority = "priority", assignee = "assignee", project = "project" }

-- What the page holds: the numbered menu that is open (a pick of an older one is not one), the
-- menu waited for until the choices come, the choices by team, the issues a change is on its way
-- for, why the last was not made, and the title being edited.
local C = { serial = 0, menu = nil, options = {}, busy = {}, error = nil, rename = nil }
change.state = C

-- --- The choices of a team ---------------------------------------------------------------------------

-- What the program said can be chosen for `team`, when it has. The choices are asked for again when the
-- lists are (the program lets them go): those held stay in use until the new ones come.
function change.options(team)
	local held = team ~= nil and C.options[team] or nil
	return held ~= nil and held.data or nil
end

-- Whether the choices of `team` are asked for or being asked, and there are none to use.
local function waiting(team)
	local held = C.options[team]
	if held == nil then
		return true
	end
	return held.data == nil and (held.status == "loading" or held.at ~= S.issues.updated)
end

-- Asks for the choices of `team` where they are not here and not on their way, as of the lists now. A
-- failed asking is made again only when `again` (the person pressed something), never by itself.
function change.ask(team, again)
	if team == nil or S.issues == nil then
		return
	end
	local held = C.options[team]
	local now = S.issues.updated
	if held ~= nil and held.at == now and (held.status == "loading" or held.status == "ready" or not again) then
		return
	end
	local kept = held ~= nil and held.data or nil
	C.options[team] = { status = "loading", at = now, data = kept }
	kit.call("new_options", { team = team }, function(result, err)
		-- (The program answers with the first team where it has not the one asked: not its choices.)
		if err or result.team ~= team then
			C.options[team] = { status = "failed", at = now, data = kept }
			return true
		end
		C.options[team] = { status = "ready", data = result, at = now }
		return true
	end)
end

-- Whether the controls of an issue of `team` can be pressed: its choices are here (or could not come,
-- and a press asks again).
function change.ready(team)
	return team ~= nil and C.options[team] ~= nil and not waiting(team)
end

-- --- The targets -------------------------------------------------------------------------------------

-- The line of the issue `id`: the list's, or the open issue's where the list has it no more.
local function row_of(id)
	return list.issue(id) or act.panel_row(id)
end

-- The ids of what a command changes: the issues selected, else the one under the cursor. Where the
-- issue has the whole pane the list is not on the page, and what is selected in it is not seen: the
-- change is of the issue that is read.
function change.targets()
	if S.alone then
		local id = act.panel_id()
		return id ~= nil and { id } or {}
	end
	local ids = list.marked_ids()
	if #ids == 0 then
		local item = list.current()
		if item ~= nil and item.kind == "issue" then
			ids = { item.row.id }
		end
	end
	return ids
end

-- Whether `id` has a change on its way.
function change.busy(id)
	return C.busy[id] == true
end
act.busy = change.busy

-- The team the issues are of: its id, "mixed" where they are of several, nil where one's is not known.
local function team_of(rows)
	local team = nil
	for _, row in rows do
		if row.teamId == nil then
			return nil
		elseif team == nil then
			team = row.teamId
		elseif team ~= row.teamId then
			return "mixed"
		end
	end
	return team
end

-- The lines of the targets, or nil where one of them is not known.
local function target_rows(ids)
	local rows = {}
	for _, id in ids do
		local row = row_of(id)
		if row == nil then
			return nil
		end
		rows[#rows + 1] = row
	end
	return rows
end

-- Whether a command can change the targets now: there are some, none has a change on its way, and
-- (where `plain`: the priority and the title need no choices) the choices of their team are here
-- (issues of several teams are offered what is the same for all, and need none).
function change.can(plain)
	local ids = change.targets()
	if #ids == 0 then
		return false
	end
	for _, id in ids do
		if C.busy[id] then
			return false
		end
	end
	local rows = target_rows(ids)
	if rows == nil then
		return false
	end
	local team = team_of(rows)
	return plain == true or team == "mixed" or change.ready(team)
end

-- What the menus mark: what all the issues have the same (nil where they differ).
local function current_of(rows)
	local first = rows[1]
	local function same(of)
		local value = of(first)
		for _, row in rows do
			if of(row) ~= value then
				return nil
			end
		end
		return value
	end
	local current = {
		state = same(function(row)
			return row.stateId
		end),
		priority = same(function(row)
			return row.priority
		end),
		assignee = same(function(row)
			return row.assigneeId or "none"
		end),
		project = same(function(row)
			return row.projectId or "none"
		end),
		labels = {},
	}
	-- A label is marked where every issue has it.
	for _, label in first.labels do
		local all = true
		for _, row in rows do
			if values.find(row.labels, label.id) == nil then
				all = false
			end
		end
		current.labels[label.id] = all or nil
	end
	return current
end

-- --- The menus ---------------------------------------------------------------------------------------

local function reason(words)
	return { key = "reason", title = words, enabled = false }
end

-- Opens the menu with `entries` for the request, near its element where that is laid out, else in the
-- middle. The reply.
local function show(request, entries)
	local where = { at = { x = S.width / 2 - 100, y = S.height / 3 } }
	if request.near ~= nil and b.layout.bounds(request.near) ~= nil then
		where = { near = request.near }
	end
	C.menu = request
	return u:menu(MENU, entries, where) or true
end

-- The key a choice comes back by: the menu it was in, the field it writes and the choice. A label
-- that every issue has is taken off, else it is put on.
local function key_of(request, choice)
	local field = FIELDS[choice.kind]
	if choice.kind == "label" then
		field = choice.on and "removeLabel" or "addLabel"
	end
	return request.serial .. "|" .. field .. "|" .. choice.id
end

-- The menu of the request, its choices being known.
local function open_menu(request)
	local rows = target_rows(request.ids)
	if rows == nil then
		return true
	end
	local entries = {}
	if #rows > 1 then
		entries[1] = { key = "head", title = TITLES[request.kind] .. " of " .. #rows .. " issues", enabled = false }
	end
	local team = team_of(rows)
	if #rows > MOST then
		entries[#entries + 1] = reason("Up to " .. MOST .. " issues change at a time")
		return show(request, entries)
	elseif team == "mixed" and request.kind ~= "priority" then
		entries[#entries + 1] = reason("The issues are of more than one team")
		return show(request, entries)
	end
	local data = request.kind == "priority" and {} or change.options(team)
	if data == nil then
		return true
	end
	local choices = values.choices(request.kind, data, current_of(rows), data.allStates)
	for _, entry in values.entries(choices, function(choice)
		return key_of(request, choice)
	end) do
		entries[#entries + 1] = entry
	end
	return show(request, entries)
end

-- Opens the menu of `kind` (`state`, `priority`, `assignee`, `labels`, `project`) for the issues `ids`
-- near the element named `near`. The reply.
function change.open(kind, ids, near)
	if #ids == 0 then
		return true
	end
	C.serial += 1
	local request = { kind = kind, ids = table.clone(ids), near = near, serial = C.serial }
	local rows = target_rows(ids)
	if rows == nil then
		return true
	end
	local team = team_of(rows)
	request.team = team
	if kind == "priority" or team == "mixed" or #rows > MOST then
		return open_menu(request)
	elseif team == nil then
		return true
	elseif change.options(team) == nil then
		-- (They are asked for before a key is pressed; a key pressed too soon does nothing but ask.)
		change.ask(team, true)
		return true
	end
	return open_menu(request)
end

-- A command of the list (`state`, `priority`, `assignee`, `labels`): the menu for the targets, near the
-- control of the issue open where it is that issue alone, else near the cursor's line.
function change.run(kind)
	local ids = change.targets()
	if #ids == 0 then
		return true
	end
	local near = nil
	local control = "panel-value:" .. kind
	if #ids == 1 and act.panel_id() == ids[1] and b.layout.bounds(control) ~= nil then
		near = control
	else
		-- The cursor's line, or where the cursor is on none of the issues that change, the first of them.
		local under = list.current()
		local on_target = false
		for _, id in ids do
			on_target = on_target or (under ~= nil and under.kind == "issue" and under.row.id == id)
		end
		near = "row:" .. (on_target and list.state.cursor or ("i:" .. ids[1]))
	end
	return change.open(kind, ids, near)
end

-- A press on a value of the issue open.
function change.press(id)
	local kind = string.match(id, "^panel%-value:(%a+)$")
	if kind == nil then
		return nil
	end
	local issue = act.panel_id()
	if issue == nil or C.busy[issue] then
		return true
	end
	return change.open(kind, { issue }, id)
end

-- --- Sending -----------------------------------------------------------------------------------------

-- The key of the group an issue is in once it is in `state` (`kind:name`, as the program names them),
-- where that group is folded or is not shown yet (Backlog and Triage begin folded): the person is
-- shown where the issue went, not a count that grew.
local function folded(state)
	-- (A filter shows what it finds: no group is folded over a match.)
	if S.issues.filter ~= "" then
		return nil
	end
	local key = state.kind .. ":" .. state.name
	if S.closed[key] == false then
		return nil
	end
	for _, group in S.issues.groups do
		if group.key == key then
			return group.closed == true and key or nil
		end
	end
	return (state.kind == "backlog" or state.kind == "triage") and key or nil
end

-- The words of a refusal where the change is of the issues the strip is about.
local function strip_words(request, words)
	local what = #request.ids == 1 and (request.what or "the issue") or (#request.ids .. " issues")
	return (string.gsub(words, "^Linear did not change it", "Linear did not change " .. what))
end

-- Sends one change: the issues `ids`, the `field` (`status`, `priority`, `assignee`, `addLabel`,
-- `removeLabel`, `project` or `title`) and its `value`. Once: a change on issues that have one on
-- its way is not sent.
local function send(ids, field, value, state)
	for _, id in ids do
		if C.busy[id] then
			return
		end
	end
	local params = { ids = table.clone(ids) }
	params[field] = value
	for _, id in ids do
		C.busy[id] = true
	end
	C.error = nil
	act.panel_hold(ids)
	local first = row_of(ids[1])
	local request = { ids = table.clone(ids), what = first ~= nil and first.identifier or nil }
	local function go()
		kit.call("change", params, function(_, err)
			if err then
				-- The program asked Linear what it holds where the change may have been made (no answer,
				-- or one that could not be read): what the open issue says is asked for again, too, for
				-- it may be of what the lists do not hold any more (done, given to someone else).
				for _, id in ids do
					C.busy[id] = nil
					if not act.panel_reask(id, function() end) then
						act.panel_forget(id)
					end
				end
				C.error = { ids = request.ids, words = err.message or err.code, strip = strip_words(request, err.message or err.code) }
				return true
			end
			-- What the open issue says is asked for again, for it may be of what the lists do not
			-- hold any more (done, given to someone else); the control is busy until it comes.
			for _, id in ids do
				if not act.panel_reask(id, function()
					C.busy[id] = nil
				end) then
					C.busy[id] = nil
					act.panel_forget(id)
				end
			end
			return true
		end)
	end
	local key = state ~= nil and folded(state) or nil
	if key ~= nil then
		S.closed[key] = false
		act.view({ closed = S.closed }, go)
	else
		go()
	end
end

-- What a pick of the menu chose: dropped where the menu is not the last opened.
local function pick(entry)
	local serial, field, id = string.match(entry, "^(%d+)|(%a+)|(.+)$")
	local request = C.menu
	if serial == nil or request == nil or tonumber(serial) ~= request.serial then
		return
	end
	C.menu = nil
	local value = id
	if id == "none" and (field == "assignee" or field == "project") then
		value = b.json.null
	end
	local state = nil
	if field == "status" then
		local data = change.options(request.team)
		state = data ~= nil and values.find(data.allStates, id) or nil
	end
	send(request.ids, field, value, state)
end

-- --- The title ---------------------------------------------------------------------------------------

local function trimmed(words)
	return string.match(words, "^%s*(.-)%s*$") or ""
end

-- Whether the title of the issue open is a field now.
function change.editing()
	return C.rename ~= nil
end

-- The title is let be: what was written is not sent.
local function leave()
	C.rename = nil
end

-- Makes the title of the issue `id` (the issue open) a field with the title selected. The reply that
-- gives it the keyboard.
function change.rename(id, title)
	if C.busy[id] then
		return true
	end
	C.rename = { id = id, state = { value = title }, original = title, seen = false, fresh = true }
	u:field(TITLE_FIELD)
	return u:focus(TITLE_FIELD)
end

-- The title of the issue `id`, as the issue open draws it: its words, or the field where it is being
-- edited. `words` draws them.
function change.title(id, words)
	local edit = C.rename
	if edit == nil or edit.id ~= id then
		return words
	end
	if edit.fresh then
		-- The field is drawn again after one that was put away, and what the host kept of it then is not
		-- known: the kit is told it does not know either, so that the title is put in it, whatever it was.
		edit.fresh = false
		u:field_text(TITLE_FIELD, false)
	end
	return b.div():no_shrink():mt(8):mb(2):child(u:input(TITLE, {
		state = edit.state,
		label = "Title",
		hint = "Title",
		select = true,
		escape = true,
	}))
end

-- The title being edited is let go where the keyboard has left it (a press elsewhere, a menu) or where
-- the issue open is another: only Enter writes it. Every event.
function change.sync()
	-- The choices of the teams of what a command would change are asked for before one is pressed (and
	-- again when the lists were): the menus open from what is here.
	local rows = target_rows(change.targets())
	for _, row in rows or {} do
		change.ask(row.teamId, false)
	end
	change.ask(act.panel_team(), false)
	local edit = C.rename
	if edit == nil then
		return
	end
	if act.panel_id() ~= edit.id then
		leave()
	elseif u:keyboard() == TITLE_FIELD then
		edit.seen = true
	elseif edit.seen then
		leave()
	end
end

act.sync = change.sync

-- --- Why not -----------------------------------------------------------------------------------------

-- The words of why the last change was not made, for the issue open, when it is that issue alone
-- (they are said under its values); else nil, and the strip says them.
function change.error_for(id)
	local error = C.error
	if error ~= nil and #error.ids == 1 and error.ids[1] == id then
		return error.words
	end
	return nil
end

-- The strip at the head of the list, where the issue open is not what was changed.
function change.strip()
	local error = C.error
	if error == nil or (#error.ids == 1 and error.ids[1] == act.panel_id()) then
		return nil
	end
	return u:banner("change-problem", {
		tone = "warn",
		icon = "assets/icons/alert-triangle.svg",
		words = error.strip,
		actions = { { key = "dismiss", title = "Dismiss" } },
	})
end

-- What the person did to the menu, the title or the strip. The reply, or nil where it was not ours.
function change.action(action)
	local from, kind = action.from, action.kind
	if from == MENU and kind == "menu" then
		pick(action.entry)
		return true
	elseif from == "change-problem" and kind == "press" then
		C.error = nil
		return true
	elseif from == TITLE then
		local edit = C.rename
		if edit == nil then
			return true
		end
		if kind == "change" then
			edit.state.value = action.text
		elseif kind == "escape" then
			leave()
			return u:focus(nil)
		elseif kind == "submit" then
			local text = trimmed(edit.state.value)
			local id = edit.id
			leave()
			if text ~= "" and text ~= edit.original then
				send({ id }, "title", text)
			end
			return u:focus(nil)
		end
		return true
	end
	return nil
end

return change
