-- The value controls of an issue (its status, priority, assignee, labels and project), once: the form of
-- a new issue and the issue open beside the list draw the same chips and open the same menus. What
-- each part adds is its own: the form keeps a draft, the issue is changed in Linear (change.lua).
--
-- A chip is what a value is now (or its name, dim, where it has none) and the mark that it opens a menu.
-- `shown` is what the chips say, `current` what the menus mark; both are built by the part that uses
-- them, from what it holds.

local icons = require("./icons")
local kit = require("../lib/kit")

local b, u, SIZE = kit.b, kit.u, kit.SIZE

local values = {}

values.PRIORITIES = {
	{ "none", "No priority" },
	{ "urgent", "Urgent" },
	{ "high", "High" },
	{ "medium", "Medium" },
	{ "low", "Low" },
}

-- The item of `items` with `id`, or nil.
function values.find(items, id)
	if id == nil then
		return nil
	end
	for _, item in items do
		if item.id == id then
			return item
		end
	end
	return nil
end

-- The person's own entry among the members, or nil.
function values.me_of(data)
	for _, member in data.members do
		if member.me then
			return member
		end
	end
	return nil
end

-- The words of a priority (`high`: High).
function values.priority_name(id)
	for _, known in values.PRIORITIES do
		if known[1] == id then
			return known[2]
		end
	end
	return id
end

-- A title with the mark of what is chosen.
function values.mark(on, words)
	return (on and "\u{2713} " or "") .. words
end

-- The kind a choice of the menu of `kind` is told by (the menu of Labels chooses one label).
local CHOICE = { labels = "label" }

-- The choices of the menu of a value, each `{ kind, id, title, on }`: `kind` says which value (`team`,
-- `state`, `priority`, `assignee`, `label`, `project`) and `id` which choice, `none` for no one and
-- no project. `current` is what is marked: `team`, `state`, `priority`, `labels` (a set of ids),
-- and `assignee` and `project` (an id, `none` where there is none, nil where it is not one thing).
-- `states` are the states offered (the form's open ones, or all of them).
function values.choices(kind, data, current, states)
	local out = {}
	local made = CHOICE[kind] or kind
	local function add(id, words, on)
		out[#out + 1] = { kind = made, id = id, title = values.mark(on, words), on = on }
	end
	if kind == "team" then
		for _, team in data.teams do
			add(team.id, team.key .. " " .. team.name, team.id == data.team)
		end
	elseif kind == "state" then
		for _, state in states or data.states do
			add(state.id, state.name, state.id == current.state)
		end
	elseif kind == "priority" then
		for _, priority in values.PRIORITIES do
			add(priority[1], priority[2], priority[1] == current.priority)
		end
	elseif kind == "assignee" then
		local me = values.me_of(data)
		if me ~= nil then
			add(me.id, "Me", me.id == current.assignee)
		end
		for _, member in data.members do
			if not member.me then
				add(member.id, member.name, member.id == current.assignee)
			end
		end
		add("none", "No one", current.assignee == "none")
	elseif kind == "labels" then
		for _, label in data.labels do
			add(label.id, label.name, current.labels[label.id] == true)
		end
	elseif kind == "project" then
		add("none", "No project", current.project == "none")
		for _, project in data.projects do
			add(project.id, project.name, project.id == current.project)
		end
	end
	return out
end

-- The entries the host's menu is given for `choices`: `key_of(choice)` is the key a pick comes back by.
function values.entries(choices, key_of)
	local out = {}
	for _, choice in choices do
		out[#out + 1] = { key = key_of(choice), title = choice.title }
	end
	return out
end

-- --- Drawing -----------------------------------------------------------------------------------------

-- A value that opens a menu: what it is now and the mark that it opens. `o.disabled`: it cannot be
-- pressed now (the choices are not there yet, or a change is on its way).
function values.chip(name, kids, label, o)
	o = o or {}
	local t = u.theme
	local live = not o.disabled
	kit.buttons[name] = live or nil
	local over = live and u:hovered(name)
	local held = over and u:pressed(name)
	local el = b.div()
		:row()
		:items_center()
		:gap(7)
		:h(SIZE.control)
		:px(9)
		:no_shrink()
		:rounded(t.radius.control)
		:bg(held and t.control_press or over and t.control_hover or t.control)
		:border(1, u:focus_visible(name) and t.ring or t.clear)
		:color(t.ink)
		:role("button")
		:label(label)
		:tip(label)
		:motion({ seconds = t.motion, color = true })
	if live then
		el:press(name):focusable()
	else
		el:disabled(true)
	end
	for _, kid in kids do
		el:child(kid)
	end
	el:child(kit.icon("chevron-down", 10, "dim"))
	return el
end

local function words(text, unset)
	return b.text(text):text_size(SIZE.small):color(unset and "dim" or "foreground"):no_shrink()
end

-- The chips of an issue's values, in a row that wraps, each named `o.prefix` and its kind
-- (`new-value:state`). `shown`: `team` (`{ key }`, the form's, where there are several), `state`
-- (`{ name, kind }`), `priority` (`high`), `assignee` (`{ name }`), `labels` (each `{ name, color }`)
-- and `project` (`{ name }`), each nil where there is none. `o`: `disabled` (and `priority_disabled`
-- where the priority needs no choices and is not disabled with the rest), `busy` (a turning picture
-- after the chips), `tail` (an element after them, that wraps with them), and whether to draw
-- `labels` and `projects` (a team with none of them has no such control).
function values.chips(shown, o)
	local row = b.div():row():wrap():gap_x(8):gap_y(8):no_shrink()
	local function chip(kind, kids, label)
		local off = o.disabled
		if kind == "priority" and o.priority_disabled ~= nil then
			off = o.priority_disabled
		end
		return values.chip(o.prefix .. kind, kids, label, { disabled = off })
	end
	if shown.team ~= nil then
		row:child(chip("team", { words(shown.team.key) }, "Team"))
	end
	row:child(chip("state", {
		icons.state(shown.state ~= nil and shown.state.kind or "other"),
		words(shown.state ~= nil and shown.state.name or "Status", shown.state == nil),
	}, "Status"))
	row:child(chip("priority", {
		icons.priority(shown.priority),
		words(shown.priority == "none" and "Priority" or values.priority_name(shown.priority), shown.priority == "none"),
	}, "Priority"))
	row:child(chip("assignee", {
		icons.person(),
		words(shown.assignee ~= nil and shown.assignee.name or "Assignee", shown.assignee == nil),
	}, "Assignee"))
	if o.labels then
		local kids = {}
		for _, label in shown.labels do
			kids[#kids + 1] = icons.dot(label.color)
			kids[#kids + 1] = words(label.name)
		end
		if #kids == 0 then
			kids = { icons.tag(), words("Labels", true) }
		end
		row:child(chip("labels", kids, "Labels"))
	end
	if o.projects then
		row:child(chip("project", {
			icons.cube(),
			words(shown.project ~= nil and shown.project.name or "Project", shown.project == nil),
		}, "Project"))
	end
	if o.busy then
		row:child(b.div():h(SIZE.control):row():items_center():child(kit.spinner(13, "dim")))
	end
	if o.tail ~= nil then
		row:child(o.tail)
	end
	return row
end

return values
